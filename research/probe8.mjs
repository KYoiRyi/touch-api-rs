const H = {'user-agent':'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36','content-type':'application/json'}
async function t(n, p, o={}) { const r = await fetch('https://pan.touchgal.net'+p, {...o, headers:{...H,...(o.headers||{})}}); const x = await r.text(); console.log(n, r.status, x.slice(0,700)); return x }
const list = await t('root', '/api/v3/share/list/1leVSX%2F')
const j = JSON.parse(list)
const objs = j.data.objects
console.log('objects:', objs.map(o=>({name:o.name, type:o.type, path:o.path})))
const dir = objs.find(o=>o.type==='dir')
if (dir) {
  await t('subdir', '/api/v3/share/list/1leVSX%2F?path=' + encodeURIComponent(dir.path === '/' ? '/' + dir.name : dir.path + '/' + dir.name))
}
const file = objs.find(o=>o.type==='file')
if (file) {
  const p = file.path === '/' ? '/' + file.name : file.path + '/' + file.name
  await t('download file', '/api/v3/share/download/1leVSX?path=' + encodeURIComponent(p), {method:'PUT'})
}
