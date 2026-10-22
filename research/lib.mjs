import fs from 'node:fs'
export const CDP = process.env.CDP_ENDPOINT || 'http://127.0.0.1:9222'
export const sleep = (ms) => new Promise(r => setTimeout(r, ms))

export async function newPage(url = 'about:blank') {
  const t = await fetch(CDP + '/json/new?' + encodeURIComponent(url), { method: 'PUT' }).then(r => r.json())
  return connect(t.webSocketDebuggerUrl, t.id)
}

function connect(wsUrl, targetId) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl)
    let id = 0
    const pending = new Map()
    const listeners = []
    const log = []
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data)
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id)
        pending.delete(msg.id)
        msg.error ? rej(new Error(JSON.stringify(msg.error))) : res(msg.result)
      } else if (msg.method) {
        for (const l of listeners) { try { l(msg) } catch {} }
      }
    }
    ws.onerror = (e) => reject(new Error('ws error ' + (e.message || '')))
    ws.onopen = () => resolve(makeApi())
    function makeApi() {
      const api = {
        targetId, log, listeners,
        send(method, params = {}) {
          const mid = ++id
          return new Promise((res, rej) => {
            pending.set(mid, { res, rej })
            ws.send(JSON.stringify({ id: mid, method, params }))
          })
        },
        on(fn) { listeners.push(fn) },
        close() { try { ws.close() } catch {} },
        async init({ net = true } = {}) {
          await api.send('Page.enable')
          await api.send('Runtime.enable')
          if (net) {
            await api.send('Network.enable', { maxTotalBufferSize: 200 * 1024 * 1024, maxResourceBufferSize: 50 * 1024 * 1024 })
            api.on(async (m) => {
              if (m.method === 'Network.responseReceived') {
                const { requestId, response, type } = m.params
                const rec = { requestId, url: response.url, status: response.status, mime: response.mimeType, type, headers: response.headers, ts: Date.now() }
                log.push(rec)
              } else if (m.method === 'Network.requestWillBeSent') {
                const req = m.params.request
                let rec = log.find(n => n.requestId === m.params.requestId)
                if (!rec) { rec = { requestId: m.params.requestId }; log.push(rec) }
                rec.method = req.method
                rec.postData = req.postData
                rec.reqHeaders = req.headers
                rec.url = rec.url || req.url
              } else if (m.method === 'Network.loadingFinished') {
                const rec = log.find(n => n.requestId === m.params.requestId)
                if (rec) rec.finished = true
              }
            })
          }
        },
        async goto(url, waitMs = 8000) {
          let loaded = false
          const f = (m) => { if (m.method === 'Page.loadEventFired') loaded = true }
          listeners.push(f)
          await api.send('Page.navigate', { url })
          const start = Date.now()
          while (!loaded && Date.now() - start < waitMs) await sleep(100)
          await sleep(1500)
        },
        async eval(expression) {
          const r = await api.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true, userGesture: true })
          if (r.exceptionDetails) throw new Error('eval error: ' + JSON.stringify(r.exceptionDetails).slice(0, 600))
          return r.result.value
        },
        async shot(path, opts = {}) {
          const r = await api.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false, ...opts })
          fs.writeFileSync(path, Buffer.from(r.data, 'base64'))
          return path
        },
        async bodyText() { return api.eval('document.body.innerText') },
        async waitFor(expr, timeout = 20000, interval = 300) {
          const start = Date.now()
          while (Date.now() - start < timeout) {
            try { if (await api.eval(expr)) return true } catch {}
            await sleep(interval)
          }
          return false
        },
        async getBody(requestId) {
          try { const r = await api.send('Network.getResponseBody', { requestId }); return r.base64Encoded ? Buffer.from(r.body, 'base64').toString('utf8') : r.body } catch (e) { return null }
        },
        async click(x, y) {
          await api.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y })
          await api.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 })
          await sleep(60)
          await api.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 })
        },
        async type(text) {
          await api.send('Input.insertText', { text })
        },
      }
      return api
    }
  })
}

export async function dumpNet(page, filter) {
  const out = []
  for (const n of page.log) {
    if (filter && !filter(n)) continue
    out.push({ status: n.status, method: n.method, url: n.url, mime: n.mime, postData: (n.postData || '').slice(0, 500) })
  }
  return out
}
