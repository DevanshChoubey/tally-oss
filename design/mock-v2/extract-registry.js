// Generates states.json and transitions.json from the registry embedded in index.html (single source).
const fs=require('fs');const O=__dirname+'/';
const html=fs.readFileSync(O+'index.html','utf8');
const reg=JSON.parse(/<script id="registry" type="application\/json">([\s\S]*?)<\/script>/.exec(html)[1]);
const head={version:reg.version,generated_from:'index.html <script id="registry">',viewport:reg.viewport,themes:reg.themes,ready:reg.ready};
fs.writeFileSync(O+'states.json',JSON.stringify({...head,url:reg.url,states:reg.states.map(({id,kind,hash,title,description})=>({id,kind,hash,title,description}))},null,1)+'\n');
const css=html.slice(0,html.indexOf('</style>'));const tok={};for(const m of css.matchAll(/(--(?:dur|ease)-[a-z-]+):([^;}]+)/g))tok[m[1]]=m[2].trim();
fs.writeFileSync(O+'transitions.json',JSON.stringify({...head,url:'index.html?freeze={ms}#{render_hash}&tx={id}&theme={theme}',
 render_hash:'"end" renders the end hash and plays the motion into it; "start" renders the start hash and plays the motion out of it',
 motion_tokens:tok,transitions:reg.transitions},null,1)+'\n');
console.log(reg.states.length,'states',reg.transitions.length,'transitions',JSON.stringify(tok));
