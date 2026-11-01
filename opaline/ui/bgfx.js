// Fond animé : dégradé bruité (fbm) aux couleurs d'Opaline, très proches, légèrement attiré par la souris.
// Shader, couleurs, intensités et halo de la maquette validée, tels quels. Ajouts : module (CSP : pas de script inline), réglage
// « Fond animé » (désactivé : une image figée, aucune boucle), pauses automatiques (fenêtre cachée ou réduite, jeu en cours),
// repli sur le fond uni si WebGL est indisponible.
const FROZEN_T = 20; // instant du motif affiché quand l'animation est coupée
let c = null, gl = null, ur, ut, um;
let animated = true, gamePaused = false, raf = 0, anim = FROZEN_T, prev = 0, last = 0;
let target = [0.5, 0.5], mouse = [0.5, 0.5];
const SCALE = 0.35; // rendu en basse résolution puis étiré : le flou est voulu

const vs = 'attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}';
const fs = `precision mediump float;
uniform vec2 r; uniform float t; uniform vec2 m;
float h(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
float n(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);
  return mix(mix(h(i),h(i+vec2(1,0)),f.x),mix(h(i+vec2(0,1)),h(i+vec2(1,1)),f.x),f.y);}
float fbm(vec2 p){float v=0.,a=.5;for(int k=0;k<5;k++){v+=a*n(p);p=p*2.03+vec2(1.7,9.2);a*=.5;}return v;}
void main(){
  vec2 uv=gl_FragCoord.xy/r; vec2 q=uv*vec2(r.x/r.y,1.);
  vec2 mm=m*vec2(r.x/r.y,1.); float d=distance(q,mm);
  vec2 warp=(q-mm)*.45*exp(-d*3.5);              // la souris tire doucement le motif
  float a=fbm(q*1.6+warp+vec2(t*.05,-t*.035));
  float b=fbm(q*2.4-warp+vec2(-t*.04,t*.06)+a);
  vec3 win=vec3(.440,.180,.220), deep=vec3(.345,.135,.175), prune=vec3(.500,.215,.300), rose=vec3(.660,.300,.340);
  vec3 col=mix(deep,win,smoothstep(.25,.75,a));
  col=mix(col,prune,smoothstep(.40,.85,b)*.55);
  col=mix(col,rose,smoothstep(.40,.95,a*b*1.8)*.7);
  col=mix(col,vec3(.95,.58,.62),.34*exp(-d*d*40.));          // halo très léger sous le curseur
  col+=(h(gl_FragCoord.xy+t)-.5)/255.*3.;         // grain : pas de bandes
  gl_FragColor=vec4(col,1.);
}`;

function setup() {
  const canvas = document.createElement('canvas');
  canvas.id = 'bgfx'; canvas.setAttribute('aria-hidden', 'true');
  document.querySelector('.window').prepend(canvas);
  const ctx = canvas.getContext('webgl', { antialias: false, premultipliedAlpha: false });
  const fail = () => { canvas.remove(); c = gl = null; return false; };
  if (!ctx) return fail();
  const sh = (type, src) => { const s = ctx.createShader(type); ctx.shaderSource(s, src); ctx.compileShader(s); return s; };
  const pr = ctx.createProgram(); ctx.attachShader(pr, sh(ctx.VERTEX_SHADER, vs)); ctx.attachShader(pr, sh(ctx.FRAGMENT_SHADER, fs)); ctx.linkProgram(pr);
  if (!ctx.getProgramParameter(pr, ctx.LINK_STATUS)) return fail();
  ctx.useProgram(pr);
  ctx.bindBuffer(ctx.ARRAY_BUFFER, ctx.createBuffer()); ctx.bufferData(ctx.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), ctx.STATIC_DRAW);
  const loc = ctx.getAttribLocation(pr, 'p'); ctx.enableVertexAttribArray(loc); ctx.vertexAttribPointer(loc, 2, ctx.FLOAT, false, 0, 0);
  const U = (k) => ctx.getUniformLocation(pr, k); ur = U('r'); ut = U('t'); um = U('m');
  c = canvas; gl = ctx;
  canvas.addEventListener('webglcontextlost', (e) => { e.preventDefault(); stop(); canvas.remove(); c = gl = null; }); // repli : fond uni
  return true;
}

function draw() {
  if (!gl) return;
  const w = Math.max(1, Math.round(c.clientWidth * SCALE)), h = Math.max(1, Math.round(c.clientHeight * SCALE));
  if (c.width !== w || c.height !== h) { c.width = w; c.height = h; gl.viewport(0, 0, w, h); }
  gl.uniform2f(ur, w, h); gl.uniform1f(ut, anim); gl.uniform2f(um, mouse[0], mouse[1]);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
}

function frame(now) {
  raf = requestAnimationFrame(frame);
  if (now - last < 33) return; // ~30 i/s
  last = now;
  anim += prev ? (now - prev) / 1000 : 0; prev = now;
  mouse = mouse.map((v, i) => v + (target[i] - v) * 0.09); // retard doux
  draw();
}

function stop() { if (raf) cancelAnimationFrame(raf); raf = 0; prev = 0; }

/** Animation en cours seulement si activée, fenêtre visible et aucun jeu lancé ; sinon une image fixe reste affichée. */
function sync() {
  if (!gl) return;
  const run = animated && !gamePaused && !document.hidden;
  if (run) { if (!raf) raf = requestAnimationFrame(frame); return; }
  stop();
  if (!animated) { anim = FROZEN_T; mouse = [0.5, 0.5]; }
  draw(); // image figée (désactivé) ou dernière image conservée (pause)
}

export function initBackground(enabled = true) {
  animated = enabled;
  if (!setup()) return;
  addEventListener('pointermove', (e) => { const b = c?.getBoundingClientRect(); if (b) target = [(e.clientX - b.left) / b.width, 1 - (e.clientY - b.top) / b.height]; });
  document.addEventListener('visibilitychange', sync);
  new ResizeObserver(() => { if (!raf) draw(); }).observe(c); // un redimensionnement efface le canvas : on redessine l'image fixe
  sync();
}

export const setAnimated = (v) => { animated = !!v; sync(); };
export const setGamePaused = (v) => { gamePaused = !!v; sync(); };
/** Pour les tests de l'interface : état courant. */
export const bgState = () => ({ ok: !!gl, running: !!raf, animated, gamePaused });
