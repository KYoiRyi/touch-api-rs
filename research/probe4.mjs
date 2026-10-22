const H = {'user-agent':'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36','content-type':'application/json'}
async function t(n, p, o={}) { const r = await fetch('https://pan.touchgal.net'+p, {...o, headers:{...H,...(o.headers||{})}}); const x = await r.text(); console.log(n, r.status, x.slice(0,260)) }
await t('single dl', '/api/v3/share/download/6X8Hp', {method:'PUT'})
await t('single list', '/api/v3/share/list/6X8Hp%2F')
await t('archive folder', '/api/v3/share/archive/5yqBTE', {method:'POST', body: JSON.stringify({items:[], dirs:[], path:'/'})})
await t('file info', '/api/v3/share/info/6X8Hp')
