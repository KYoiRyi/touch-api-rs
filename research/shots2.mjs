import { newPage, sleep } from './lib.mjs'
const c = await newPage('about:blank')
await c.init()
await c.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false })
const shots = '/var/lib/dsh/Touchgal/shots'
await c.goto('https://www.touchgal.ink/a555829b', 12000)
await sleep(4000)
await c.shot(shots + '/0-patch-detail.png')
// advanced filter panel on search page
await c.goto('https://www.touchgal.ink/search', 10000)
await sleep(2000)
await c.eval("(()=>{const b=[...document.querySelectorAll('button')].find(x=>x.innerText.includes('高级筛选')); b&&b.click()})()")
await sleep(1500)
await c.shot(shots + '/5-advanced-filters.png')
c.close(); process.exit(0)
