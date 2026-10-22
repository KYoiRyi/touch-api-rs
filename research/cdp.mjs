// Minimal CDP client for exploration (Node 24 global WebSocket)
import fs from 'node:fs'

const CDP = process.env.CDP_ENDPOINT || 'http://127.0.0.1:9222'

export async function listPages() {
  return fetch(CDP + '/json/list').then(r => r.json())
}

export async function newPage(url) {
  const t = await fetch(CDP + '/json/new?' + encodeURIComponent(url || 'about:blank'), { method: 'PUT' }).then(r => r.json())
  const c = await connect(t.webSocketDebuggerUrl)
  c.targetId = t.id
  return c
}

export async function connectPage(wsUrl) {
  return connect(wsUrl)
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl)
    let id = 0
    const pending = new Map()
    const listeners = []
    const net = []
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data)
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id)
        pending.delete(msg.id)
        msg.error ? rej(new Error(JSON.stringify(msg.error))) : res(msg.result)
      } else if (msg.method) {
        if (msg.method === 'Network.responseReceived') {
          const { requestId, response, type } = msg.params
          net.push({ requestId, url: response.url, status: response.status, mime: response.mimeType, type, headers: response.headers })
        }
        if (msg.method === 'Network.requestWillBeSent') {
          const e = net.find(n => n.requestId === msg.params.requestId)
          const req = msg.params.request
          const rec = e || { requestId: msg.params.requestId }
          rec.method = req.method
          rec.postData = req.postData
          rec.reqHeaders = req.headers
          rec.url = rec.url || req.url
          if (!e) net.push(rec)
        }
        if (msg.method === 'Network.loadingFinished') {
          const e = net.find(n => n.requestId === msg.params.requestId)
          if (e) e.finished = true
        }
        for (const l of listeners) l(msg)
      }
    }
    ws.onerror = (e) => reject(new Error('ws error ' + e.message))
    ws.onopen = () => {
      resolve({
        send(method, params = {}) {
          const mid = ++id
          return new Promise((res, rej) => {
            pending.set(mid, { res, rej })
            ws.send(JSON.stringify({ id: mid, method, params }))
          })
        },
        on(fn) { listeners.push(fn) },
        net,
        close() { try { ws.close() } catch {} },
        async init() {
          await this.send('Page.enable')
          await this.send('Runtime.enable')
          await this.send('Network.enable')
        },
        async goto(url, waitMs = 5000) {
          const loaded = new Promise(r => { const f = (m) => { if (m.method === 'Page.loadEventFired') { r() } }; listeners.push(f) })
          await this.send('Page.navigate', { url })
          await Promise.race([loaded, sleep(waitMs)])
          await sleep(1200)
        },
        async eval(expression, awaitPromise = true) {
          const r = await this.send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true, userGesture: true, awaitPromise })
          if (r.exceptionDetails) throw new Error('eval error: ' + JSON.stringify(r.exceptionDetails).slice(0, 400))
          return r.result.value
        },
        async shot(path) {
          const r = await this.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
          fs.writeFileSync(path, Buffer.from(r.data, 'base64'))
          return path
        },
        async waitFor(expr, timeout = 15000, interval = 300) {
          const start = Date.now()
          while (Date.now() - start < timeout) {
            try { if (await this.eval(expr)) return true } catch {}
            await sleep(interval)
          }
          return false
        },
        async bodyText() { return this.eval('document.body.innerText') },
        async click(selector) {
          return this.eval(`(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return 'NOTFOUND'; el.click(); return 'OK' })()`)
        },
        async clickByText(text) {
          return this.eval(`(() => { const els = [...document.querySelectorAll('a,button,div,span')]; const el = els.find(e => e.innerText && e.innerText.trim() === ${JSON.stringify(text)}); if (!el) return 'NOTFOUND'; el.click(); return 'OK:' + el.tagName })()`)
        },
      })
    }
  })
}

export const sleep = (ms) => new Promise(r => setTimeout(r, ms))
