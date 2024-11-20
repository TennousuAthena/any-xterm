use clap::Parser;
use futures::{stream::SplitSink, SinkExt, StreamExt};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader}, // 只使用 tokio 的 IO 类型
    net::{TcpListener, TcpStream},
    process::Command as TokioCommand,
    sync::Mutex,
};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 要执行的命令
    #[arg(short, long, default_value = "top -b")]
    command: String,

    /// WebSocket 服务器地址
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    addr: String,
}

// 定义一个结构来存储输出历史
struct OutputHistory {
    lines: VecDeque<String>,
    max_lines: usize,
}

impl OutputHistory {
    fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max_lines),
            max_lines,
        }
    }

    fn add_line(&mut self, line: String) {
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    fn get_all(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

// 定义服务器状态结构
struct ServerState {
    history: OutputHistory,
    clients: HashMap<String, SplitSink<WebSocketStream<TcpStream>, Message>>,
}

impl ServerState {
    fn new(max_lines: usize) -> Self {
        Self {
            history: OutputHistory::new(max_lines),
            clients: HashMap::new(),
        }
    }

    async fn broadcast(&mut self, message: String) {
        // 保存到历史记录
        self.history.add_line(message.clone());

        // 广播到所有客户端
        let mut disconnected_clients = Vec::new();

        for (client_id, sender) in self.clients.iter_mut() {
            if let Err(_) = sender.send(Message::Text(message.clone())).await {
                disconnected_clients.push(client_id.clone());
            }
        }

        // 移除断开连接的客户端
        for client_id in disconnected_clients {
            self.clients.remove(&client_id);
            println!("移除断开的客户端: {}", client_id);
        }
    }

    async fn clear_history(&mut self) {
        // 清空历史记录
        self.history.lines.clear();
        // 发送清屏命令给所有客户端
        self.broadcast("\x1b[2J\x1b[H".to_string()).await;
    }
}

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args = Args::parse();

    // 创建共享的输出历史
    let state = Arc::new(Mutex::new(ServerState::new(1000))); // 保存最近1000行
    let state_clone = state.clone();

    // 启动命令执行任务
    let command = args.command.clone();
    tokio::spawn(async move {
        run_command(command, state_clone).await;
    });

    // 设置 WebSocket 服务器
    let listener = TcpListener::bind(&args.addr).await.expect("无法绑定地址");
    println!("WebSocket 服务器运行在: {}", args.addr);
    println!("执行命令: {}", args.command);

    while let Ok((stream, _)) = listener.accept().await {
        let state = state.clone();
        tokio::spawn(async move {
            handle_connection(stream, state).await;
        });
    }
}

async fn run_command(command: String, state: Arc<Mutex<ServerState>>) {
    loop {
        let mut child = TokioCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("命令执行失败");

        let stdout = child.stdout.take().expect("无法获取标准输出");
        let stderr = child.stderr.take().expect("无法获取标准错误");

        // 修改标准输出读取逻辑
        let state_clone = state.clone();
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut reader = reader.lines();
            while let Ok(Some(line_content)) = reader.next_line().await {
                println!("{}", line_content);
                state_clone.lock().await.broadcast(line_content).await;
            }
        });

        // 修改标准错误读取逻辑
        let state_clone = state.clone();
        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut reader = reader.lines();
            while let Ok(Some(line_content)) = reader.next_line().await {
                println!("错误: {}", line_content);
                state_clone
                    .lock()
                    .await
                    .broadcast(format!("错误: {}", line_content))
                    .await;
            }
        });

        // 等待任意一个任务完成
        tokio::select! {
            _ = stdout_task => {},
            _ = stderr_task => {},
        }

        // 终止子进程
        let _ = child.kill().await;

        // 等待10s时间后重新启动
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        // 清空屏幕和历史记录
        state.lock().await.clear_history().await;
    }
}

async fn handle_connection(stream: TcpStream, state: Arc<Mutex<ServerState>>) {
    let addr = stream.peer_addr().expect("无法获取对等地址");
    println!("新的客户端连接: {}", addr);

    let ws_stream = accept_async(stream).await.expect("WebSocket 握手失败");
    println!("WebSocket 连接已建立: {}", addr);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let client_id = Uuid::new_v4().to_string();

    // 发送历史记录
    {
        let state_lock = state.lock().await;
        let history_lines = state_lock.history.get_all();
        for line in history_lines {
            if let Err(_) = ws_sender.send(Message::Text(line)).await {
                println!("发送历史记录失败: {}", addr);
                return;
            }
        }
    }

    // 添加到客户端列表
    {
        let mut state_lock = state.lock().await;
        state_lock.clients.insert(client_id.clone(), ws_sender);
        println!("当前连接数: {}", state_lock.clients.len());
    }

    // 等待客户端断开连接
    while let Some(msg) = ws_receiver.next().await {
        if let Ok(msg) = msg {
            if msg.is_close() {
                break;
            }
        }
    }

    // 移除客户端
    {
        let mut state_lock = state.lock().await;
        state_lock.clients.remove(&client_id);
        println!(
            "客户端断开连接: {}，当前连接数: {}",
            addr,
            state_lock.clients.len()
        );
    }
}
