const H = {'user-agent':'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36','content-type':'application/json'}
async function t(n, p, o={}) { const r = await fetch('https://pan.touchgal.net'+p, {...o, headers:{...H,...(o.headers||{})}}); const x = await r.text(); console.log(n, r.status, x.slice(0,300)) }
// archive all items of folder share
await t('archive items', '/api/v3/share/archive/5yqBTE', {method:'POST', body: JSON.stringify({items:['JlLD0Fo','52VyEIn'], dirs:[], path:'/'})})
await t('archive single', '/api/v3/share/archive/5yqBTE', {method:'POST', body: JSON.stringify({items:['JlLD0Fo'], dirs:[], path:'/'})})
// subfolder listing test: find a share with subfolders (search patches for pan links with dirs)
