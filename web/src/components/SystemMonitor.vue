<template>
    <div class="terminal-container" ref="terminalContainer">
        <div ref="terminal" class="terminal"></div>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from 'xterm-addon-fit'
import '@xterm/xterm/css/xterm.css'

const terminal = ref<HTMLElement | null>(null)
const terminalContainer = ref<HTMLElement | null>(null)
const term = ref<Terminal | null>(null)
const ws = ref<WebSocket | null>(null)
const reconnectAttempts = ref(0)
const maxReconnectAttempts = 99
const baseDelay = 1000 // 初始延迟1秒
let reconnectTimeout: number | null = null

onMounted(() => {
    // 初始化 xterm
    term.value = new Terminal({
        cursorBlink: true,
        fontSize: 14,
        fontFamily: 'monospace',
        theme: {
            background: '#1e1e1e',
            foreground: '#d4d4d4',
            cursor: '#d4d4d4'
        }
    })

    const fitAddon = new FitAddon()
    term.value.loadAddon(fitAddon)

    if (terminal.value) {
        term.value.open(terminal.value)
        fitAddon.fit()
    }

    // 写入欢迎信息
    term.value.writeln('[AnyXterm] \x1b[36mWelcome to AnyXterm!\x1b[0m')
    term.value.writeln('[AnyXterm] \x1b[90mConnecting to WebSocket server...\x1b[0m')

    // 创建 WebSocket 连接
    initWebSocket()

    // 监听窗口大小变化
    const handleResize = () => {
        fitAddon.fit()
    }
    window.addEventListener('resize', handleResize)

    onBeforeUnmount(() => {
        window.removeEventListener('resize', handleResize)
        if (reconnectTimeout) {
            window.clearTimeout(reconnectTimeout)
        }
    })
})

const initWebSocket = () => {
    ws.value = new WebSocket('ws://127.0.0.1:8080')

    ws.value.onopen = () => {
        term.value?.clear() // 清空屏幕
        term.value?.writeln('[AnyXterm] \x1b[32mWebSocket connected\x1b[0m')
        reconnectAttempts.value = 0 // 重置重连次数
    }

    ws.value.onclose = () => {
        term.value?.writeln('[AnyXterm] \x1b[31mWebSocket disconnected\x1b[0m')
        attemptReconnect()
    }

    ws.value.onerror = (error) => {
        term.value?.writeln('[AnyXterm] \x1b[31mWebSocket connection error\x1b[0m')
        console.error('WebSocket error:', error)
    }

    ws.value.onmessage = (event) => {
        term.value?.writeln(event.data)
    }
}

const attemptReconnect = () => {
    if (reconnectAttempts.value >= maxReconnectAttempts) {
        term.value?.writeln('[AnyXterm] \x1b[31mReached maximum reconnection attempts, please refresh the page to try again\x1b[0m')
        return
    }

    const delay = baseDelay * Math.pow(2, reconnectAttempts.value);
    term.value?.writeln(`[AnyXterm] \x1b[33mWill try to reconnect in ${delay / 1000} seconds (${reconnectAttempts.value + 1}/${maxReconnectAttempts})\x1b[0m`)

    if (reconnectTimeout) {
        window.clearTimeout(reconnectTimeout)
    }

    reconnectTimeout = window.setTimeout(() => {
        reconnectAttempts.value++
        term.value?.writeln('[AnyXterm] \x1b[90mTrying to reconnect...\x1b[0m')
        initWebSocket()
    }, delay)
}

onBeforeUnmount(() => {
    if (reconnectTimeout) {
        window.clearTimeout(reconnectTimeout)
    }
    ws.value?.close()
    term.value?.dispose()
})
</script>

<style>
body {
    margin: 0;
    padding: 0;
    overflow: hidden;
}
</style>

<style scoped>
.terminal-container {
    width: 100vw;
    height: 100vh;
    background-color: #1e1e1e;
    display: flex;
}

.terminal {
    flex: 1;
}

.terminal-container :deep(.xterm) {
    height: 100%;
}

.terminal-container :deep(.xterm-screen) {
    width: 100% !important;
    height: 100% !important;
}
</style>