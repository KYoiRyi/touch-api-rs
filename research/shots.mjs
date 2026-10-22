import { newPage, sleep } from './lib.mjs'
const c = await newPage('about:blank')
await c.init()
await c.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false })
const shots = '/var/lib/dsh/Touchgal/shots'
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36'
await c.send('Emulation.setUserAgentOverride', { userAgent: UA })

// 1 search page with results
await c.goto('https://www.touchgal.ink/search', 12000)
await sleep(2000)
await c.eval("(()=>{const i=document.querySelector('input'); const s=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value').set; s.call(i,'千恋万花'); i.dispatchEvent(new Event('input',{bubbles:true})); i.focus();})()")
await sleep(2000)
await c.send('Input.dispatchKeyEvent', {type:'keyDown', key:'Enter', code:'Enter', windowsVirtualKeyCode:13, nativeVirtualKeyCode:13})
await c.send('Input.dispatchKeyEvent', {type:'keyUp', key:'Enter', code:'Enter', windowsVirtualKeyCode:13, nativeVirtualKeyCode:13})
await sleep(6000)
await c.shot(shots + '/1-search-results.png')

// 2 resource tab with expanded official links
await c.goto('https://www.touchgal.ink/a555829b?tab=resources', 12000)
await sleep(4000)
await c.eval("(()=>{const b=[...document.querySelectorAll('button')].filter(x=>(x.getAttribute('aria-label')||'').includes('下载')); b[0]&&b[0].click()})()")
await sleep(2000)
await c.shot(shots + '/2-official-resource-links.png')

// 3 redirect interstitial
await c.eval("(()=>{const a=[...document.querySelectorAll('a')].find(x=>(x.getAttribute('href')||'').includes('pan.touchgal.net')); a&&a.click()})()")
await sleep(3500)
await c.shot(shots + '/3-redirect-interstitial.png')

// 4 pan share listing
await c.goto('https://pan.touchgal.net/s/5yqBTE', 12000)
await sleep(3000)
await c.shot(shots + '/4-pan-share-listing.png')
c.close(); process.exit(0)
