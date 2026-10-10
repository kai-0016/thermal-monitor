pub fn html() -> String {
    PAGE.to_string()
}

const PAGE: &str = r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>温度 · 占用 · 升温归因</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{background:#0d1117;color:#e6edf3;font:14px/1.5 -apple-system,"Segoe UI","Microsoft YaHei",sans-serif;padding:18px 20px 40px}
header{display:flex;align-items:baseline;gap:14px;flex-wrap:wrap;margin-bottom:16px}
h1{font-size:19px;font-weight:600;letter-spacing:.5px}
.meta{color:#7d8590;font-size:12px}
.dot{display:inline-block;width:7px;height:7px;border-radius:50%;background:#3fb950;margin-right:6px;vertical-align:middle}
.kpis{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin-bottom:16px}
.kpi{background:#161b22;border:1px solid #21262d;border-radius:10px;padding:12px 14px}
.kpi .lbl{color:#8b949e;font-size:12px;margin-bottom:6px}
.kpi .val{font-size:26px;font-weight:600;font-variant-numeric:tabular-nums;line-height:1.15}
.kpi .sub{color:#6e7681;font-size:11px;margin-top:3px}
.c-green{color:#3fb950}.c-amber{color:#d29922}.c-red{color:#f85149}.c-blue{color:#58a6ff}
.panel{background:#161b22;border:1px solid #21262d;border-radius:10px;padding:14px 16px;margin-bottom:16px}
.panel h2{font-size:13px;font-weight:600;color:#8b949e;margin-bottom:12px;letter-spacing:.4px}
.row{display:grid;grid-template-columns:1fr 1fr;gap:16px}
@media(max-width:900px){.row{grid-template-columns:1fr}}
canvas{width:100%;display:block}
.legend{display:flex;gap:16px;font-size:12px;color:#8b949e;margin-top:8px;flex-wrap:wrap}
.legend i{display:inline-block;width:10px;height:3px;border-radius:2px;margin-right:5px;vertical-align:middle}
.cores{display:grid;grid-template-columns:repeat(auto-fill,minmax(76px,1fr));gap:8px}
.core{background:#0d1117;border:1px solid #21262d;border-radius:7px;padding:8px;text-align:center}
.core .n{font-size:11px;color:#6e7681}
.core .u{font-size:15px;font-weight:600;font-variant-numeric:tabular-nums;margin:1px 0}
.core .f{font-size:10px;color:#6e7681}
.bar{height:3px;border-radius:2px;background:#21262d;margin-top:5px;overflow:hidden}
.bar>div{height:100%;border-radius:2px;transition:width .3s}
.proc{display:flex;align-items:center;gap:10px;margin-bottom:7px;font-size:12px}
.proc .nm{width:150px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#c9d1d9}
.proc .tr{flex:1;height:14px;background:#0d1117;border-radius:3px;overflow:hidden}
.proc .tr>div{height:100%;background:linear-gradient(90deg,#1f6feb,#58a6ff);border-radius:3px;transition:width .3s}
.proc .pc{width:52px;text-align:right;color:#8b949e;font-variant-numeric:tabular-nums}
.ev{border-left:3px solid #f85149;padding:10px 14px;background:#0d1117;border-radius:0 7px 7px 0;margin-bottom:10px}
.ev .head{display:flex;align-items:center;gap:10px;font-size:12px;color:#8b949e;margin-bottom:5px}
.ev .tmp{color:#f85149;font-weight:600;font-size:13px}
.ev .vd{color:#c9d1d9;font-size:13px;margin-bottom:6px}
.chips{display:flex;gap:6px;flex-wrap:wrap}
.chip{background:#1f2428;border:1px solid #30363d;border-radius:20px;padding:2px 9px;font-size:11px;color:#c9d1d9}
.chip b{color:#58a6ff;font-weight:600}
.kv{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:6px;font-size:11px;color:#6e7681;margin-top:6px}
.warn{background:#3d1d0e;border:1px solid #9e6a03;color:#f0b72f;padding:9px 12px;border-radius:7px;font-size:12px;margin-bottom:14px}
.empty{color:#6e7681;font-size:12px;padding:14px 0}
</style>
</head>
<body>
<header>
  <h1>温度 · 占用 · 升温归因</h1>
  <div class="meta"><span class="dot"></span><span id="clock">--:--:--</span> &nbsp;·&nbsp; <span id="uptime"></span></div>
</header>

<div id="srcwarn"></div>

<div class="kpis">
  <div class="kpi"><div class="lbl">温度</div><div class="val" id="k-temp">--</div><div class="sub" id="s-temp">—</div></div>
  <div class="kpi"><div class="lbl">CPU 占用</div><div class="val" id="k-cpu">--</div><div class="sub" id="s-cpu">—</div></div>
  <div class="kpi"><div class="lbl">平均频率</div><div class="val" id="k-freq">--</div><div class="sub" id="s-freq">MHz</div></div>
  <div class="kpi"><div class="lbl">内核态 DPC/中断</div><div class="val" id="k-dpc">--</div><div class="sub">驱动活动强度</div></div>
  <div class="kpi"><div class="lbl">内存</div><div class="val" id="k-mem">--</div><div class="sub">—</div></div>
  <div class="kpi"><div class="lbl">磁盘</div><div class="val" id="k-disk">--</div><div class="sub">KB/s</div></div>
  <div class="kpi"><div class="lbl">网络</div><div class="val" id="k-net">--</div><div class="sub">KB/s</div></div>
  <div class="kpi"><div class="lbl">功耗</div><div class="val" id="k-pw">--</div><div class="sub" id="s-pw">需 HWiNFO</div></div>
</div>

<div class="panel">
  <h2>温度趋势（最近 10 分钟）</h2>
  <canvas id="c1"></canvas>
  <div class="legend"><span><i style="background:#f0883e"></i>温度 ℃</span><span id="r1">—</span></div>
</div>

<div class="panel">
  <h2>CPU 占用 与 频率</h2>
  <canvas id="c2"></canvas>
  <div class="legend"><span><i style="background:#58a6ff"></i>CPU 占用 %</span><span><i style="background:#3fb950"></i>频率 MHz</span><span id="r2">—</span></div>
</div>

<div class="row">
  <div class="panel"><h2>每核心负载 / 频率</h2><div class="cores" id="cores"></div></div>
  <div class="panel"><h2>进程占用排行</h2><div id="procs"></div></div>
</div>

<div class="panel">
  <h2>升温归因事件（自动检测，按时间倒序）</h2>
  <div id="events"></div>
</div>

<script>
let S=null,C=null,E=[],UPT=0;
function fmt(v,d){return v==null?'--':v.toFixed(d===undefined?1:d)}
function tcol(t){return t==null?'':t>=80?'c-red':t>=68?'c-amber':'c-green'}
function ucol(u){return u>=70?'c-red':u>=35?'c-amber':'c-blue'}

async function tick(){
  try{
    const r=await fetch('/api/all?n=600');
    const d=await r.json();
    C=d.cur;S=d.series;E=d.events;UPT=d.uptime;
    render();
  }catch(e){}
}

function fmtDur(s){
  const h=Math.floor(s/3600),m=Math.floor(s%3600/60);
  return h>0?`已运行 ${h} 小时 ${m} 分`:`已运行 ${m} 分 ${s%60} 秒`;
}

function render(){
  if(!C){return}
  document.getElementById('clock').textContent=C.time;
  document.getElementById('uptime').textContent=fmtDur(UPT);
  const tEl=document.getElementById('k-temp');
  tEl.textContent=C.temp==null?'--':fmt(C.temp,1)+'℃';
  tEl.className='val '+tcol(C.temp);
  document.getElementById('s-temp').textContent=C.source||'';
  const cEl=document.getElementById('k-cpu');
  cEl.textContent=fmt(C.cpu,1)+'%';
  cEl.className='val '+ucol(C.cpu);
  document.getElementById('s-cpu').textContent='用户 '+fmt(C.user,1)+'% / 内核 '+fmt(C.kernel,1)+'%';
  document.getElementById('k-freq').textContent=fmt(C.freq,0);
  const dEl=document.getElementById('k-dpc');
  dEl.textContent=fmt(C.dpc+C.intr,2)+'%';
  dEl.className='val '+((C.dpc+C.intr)>1.5?'c-red':(C.dpc+C.intr)>0.5?'c-amber':'');
  document.getElementById('k-mem').textContent=fmt(C.mem,0)+'%';
  document.getElementById('k-disk').textContent=fmt(C.disk,0);
  document.getElementById('k-net').textContent=fmt(C.net,0);
  const pEl=document.getElementById('k-pw');
  if(C.power!=null){pEl.textContent=fmt(C.power,1)+'W';document.getElementById('s-pw').textContent='HWiNFO 实时'}
  else{pEl.textContent='--'}
  document.getElementById('s-freq').textContent=C.cores?C.cores.length+' 个逻辑核心':'MHz';

  const w = document.getElementById('srcwarn');
  if(C.source && C.source.indexOf('热区')>=0){
    w.innerHTML='<div class="warn">当前温度为 <b>ACPI 主板热区传感器</b>读数：绝对值一般比 CPU 裸片高几度，但<b>变化趋势和归因结论准确</b>。'+
      '若需 CPU 内核级 Tctl/Tdie 温度与实时功耗，安装 HWiNFO → Settings → 勾选 <b>Shared Memory Support</b>，本程序会自动切换到高精度数据源。</div>';
  } else { w.innerHTML=''; }

  // 核心
  let ch='';
  C.cores.forEach((c,i)=>{
    const u=Math.max(0,Math.min(100,c[0]));
    const col=u>=70?'#f85149':u>=35?'#d29922':'#238636';
    ch+=`<div class="core"><div class="n">C${i}</div><div class="u">${fmt(c[0],0)}%</div><div class="f">${fmt(c[1],0)}MHz</div><div class="bar"><div style="width:${u}%;background:${col}"></div></div></div>`;
  });
  document.getElementById('cores').innerHTML=ch;

  // 进程
  let ph='';
  if(!C.procs.length){ph='<div class="empty">当前没有明显占用 CPU 的进程</div>'}
  C.procs.forEach(p=>{
    const w=Math.min(100,p[1]/25*100);
    ph+=`<div class="proc"><div class="nm" title="${p[0]}">${p[0]}</div><div class="tr"><div style="width:${w}%"></div></div><div class="pc">${fmt(p[1],1)}%</div></div>`;
  });
  document.getElementById('procs').innerHTML=ph;

  // 事件
  let eh='';
  if(!E.length){eh='<div class="empty">暂未检测到升温事件。当温度在短时间内上升超过 2℃ 时，这里会自动给出归因结论。</div>'}
  E.forEach(e=>{
    let cs='';
    e.c.forEach(c=>{cs+=`<span class="chip">${c[0]} <b>${fmt(c[1],1)}%</b>${c[2]>0.5?' ↑'+fmt(c[2],1):''}</span>`});
    eh+=`<div class="ev"><div class="head"><b>${e.time}</b><span class="tmp">+${fmt(e.delta,1)}℃ （${fmt(e.from,1)} → ${fmt(e.to,1)}）</span>
      <span>负载滞后 ${e.lag}s</span><span>可信度 ${fmt(e.conf*100,0)}%</span><span>当时 CPU ${fmt(e.cpu,1)}%（基线 ${fmt(e.cpuBase,1)}%）</span></div>
      <div class="vd">${e.verdict}</div><div class="chips">${cs||'<span class="chip">无显著进程</span>'}</div>
      <div class="kv"><span>频率：${fmt(e.freqBase,0)} → ${fmt(e.freq,0)} MHz</span><span>内核 DPC/中断：${fmt(e.dpc,2)}%</span></div></div>`;
  });
  document.getElementById('events').innerHTML=eh;

  draw();
}

function setupCanvas(id,h){
  const cv=document.getElementById(id);
  const dpr=window.devicePixelRatio||1;
  const w=cv.clientWidth;
  if(cv.width!==Math.round(w*dpr)||cv.height!==Math.round(h*dpr)){
    cv.width=Math.round(w*dpr);cv.height=Math.round(h*dpr);
  }
  const g=cv.getContext('2d');
  g.setTransform(dpr,0,0,dpr,0,0);
  g.clearRect(0,0,w,h);
  return {g,w,h};
}

function grid(g,w,h,pad,rows,fmtL,lo,hi,color){
  g.strokeStyle='#21262d';g.fillStyle='#6e7681';g.font='10px sans-serif';g.lineWidth=1;
  for(let i=0;i<=rows;i++){
    const y=pad.t+(h-pad.t-pad.b)*i/rows;
    g.beginPath();g.moveTo(pad.l,y);g.lineTo(w-pad.r,y);g.stroke();
    const v=hi-(hi-lo)*i/rows;
    g.fillText(fmtL(v),2,y+3);
  }
}

function line(g,xs,ys,color,fill){
  if(ys.length<2)return;
  if(fill){
    const gr=g.createLinearGradient(0,0,0,ys[0].h);gr.addColorStop(0,color+'55');gr.addColorStop(1,color+'00');
    g.beginPath();g.moveTo(xs[0],ys[0].y);
    ys.forEach(p=>g.lineTo(p.x,p.y));
    g.lineTo(xs[xs.length-1],ys[0].h);g.lineTo(xs[0],ys[0].h);g.closePath();
    g.fillStyle=gr;g.fill();
  }
  g.beginPath();
  let started=false;
  ys.forEach((p,i)=>{if(!started){g.moveTo(p.x,p.y);started=true}else{g.lineTo(p.x,p.y)}});
  g.strokeStyle=color;g.lineWidth=1.6;g.stroke();
}

function mapRange(vals,y0,y1,pad){
  let lo=Math.min(...vals),hi=Math.max(...vals);
  if(!isFinite(lo)||!isFinite(hi))return null;
  if(hi-lo<1e-6){lo-=1;hi+=1}else{const p=(hi-lo)*0.15;lo-=p;hi+=p}
  const h=(y1-y0);
  return {y:y=>y1-(y-lo)/(hi-lo)*h,lo:lo,hi:hi};
}

function draw(){
  if(!S||!S.t||!S.t.length)return;
  const n=S.t.length;
  const pad={l:44,r:12,t:10,b:18};

  // 图1 温度
  {
    const {g,w,h}=setupCanvas('c1',170);
    const vals=S.temp.filter(v=>v!=null);
    if(vals.length>1){
      const R=mapRange(vals,pad.t,h-pad.b,pad);
      grid(g,w,h,pad,4,v=>v.toFixed(0)+'℃',R.lo,R.hi);
      const pts=[];const xs=[];
      for(let i=0;i<n;i++){
        if(S.temp[i]==null)continue;
        xs.push(pad.l+(w-pad.l-pad.r)*i/(n-1));
        pts.push({x:pad.l+(w-pad.l-pad.r)*i/(n-1),y:R.y(S.temp[i]),h:h-pad.b});
      }
      line(g,xs,pts,'#f0883e',true);
      document.getElementById('r1').textContent=`区间 ${R.lo.toFixed(1)} ~ ${R.hi.toFixed(1)} ℃`;
      // x 轴时间
      g.fillStyle='#6e7681';g.font='10px sans-serif';
      for(let i=0;i<=4;i++){
        const idx=Math.floor((n-1)*i/4);
        const sec=n-1-idx;
        const x=pad.l+(w-pad.l-pad.r)*idx/(n-1);
        g.fillText(sec===0?'现在':'-'+sec+'s',Math.min(x,w-pad.r-26),h-5);
      }
    }
  }
  // 图2 CPU + 频率
  {
    const {g,w,h}=setupCanvas('c2',150);
    grid(g,w,h,pad,4,v=>v.toFixed(0)+'%',0,100);
    let maxF=Math.max(...S.freq,1)*1.15;
    const cpu=[],frq=[],xs=[];
    for(let i=0;i<n;i++){
      const x=pad.l+(w-pad.l-pad.r)*i/(n-1);
      xs.push(x);
      cpu.push({x:x,y:pad.t+(h-pad.t-pad.b)*(1-Math.max(0,Math.min(100,S.cpu[i]))/100)});
      frq.push({x:x,y:pad.t+(h-pad.t-pad.b)*(1-Math.max(0,Math.min(maxF,S.freq[i]))/maxF)});
    }
    line(g,xs,cpu,'#58a6ff',true);
    line(g,xs,frq,'#3fb950',false);
    g.fillStyle='#6e7681';g.font='10px sans-serif';g.textAlign='right';
    for(let i=0;i<=4;i++){
      const y=pad.t+(h-pad.t-pad.b)*i/4;
      g.fillStyle='#3fb950';g.fillText((maxF-(maxF)*i/4).toFixed(0),w-4,y+3);
    }
    g.textAlign='left';
    document.getElementById('r2').textContent=`频率上限 ${maxF.toFixed(0)} MHz`;
  }
}

window.addEventListener('resize',draw);
tick();setInterval(tick,1000);
</script>
</body>
</html>"##;
