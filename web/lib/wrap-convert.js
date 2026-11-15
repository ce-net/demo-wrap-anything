// sdk/ts-web/WrapConvert.ts
var API_NAME = "wrap.convert";
var API_VERSION = 1;
var SCHEMA = 3658893440286991820n;
var NETFIND_SCHEMA = 16424429642170228184n;
var DEADLINE_MS = 1e3;
var CALL_DEADLINE_MS = 8e3;
var SPAWN_WAIT_MS = 1e4;
var DOOR_PORT = 7700;
var HEADER = 17;
var MAX_PAYLOAD = 16 << 20;
var K_HELLO = 0;
var K_REQ = 1;
var K_RESP = 2;
var K_ERR = 3;
var K_SUB = 5;
var K_ITEM = 6;
var K_END = 7;
var WireError = class extends Error {
};
var Timeout = class extends WireError {
};
var Closed = class extends WireError {
};
var DecodeError = class extends WireError {
};
var Unreachable = class extends Error {
  api;
  major;
  find;
  tried;
  cause;
  constructor(api, major, find, tried, cause) {
    super(
      find && find.tag === "StartFailed" ? `${api}@${major} could not start: ${find.cell} failed (${find.dep_chain.join(" -> ")}): ${find.cause}` : find ? `${api}@${major} not reachable: net.find ${find.tag}; ${cause}` : `${api}@${major} not reachable: ${cause}`
    );
    this.api = api;
    this.major = major;
    this.find = find;
    this.tried = tried;
    this.cause = cause;
  }
};
var Refused = class extends WireError {
  kind;
  detail;
  extra;
  constructor(kind, detail, extra = {}) {
    super(detail === void 0 ? kind : `${kind}: ${String(detail)}`);
    this.kind = kind;
    this.detail = detail;
    this.extra = extra;
  }
};
var W = class {
  buf = new Uint8Array(64);
  len = 0;
  room(n) {
    if (this.len + n <= this.buf.length) return;
    const nb = new Uint8Array(Math.max(this.buf.length * 2, this.len + n));
    nb.set(this.buf.subarray(0, this.len));
    this.buf = nb;
  }
  byte(b) {
    this.room(1);
    this.buf[this.len++] = b;
  }
  bytes(b) {
    this.room(b.length);
    this.buf.set(b, this.len);
    this.len += b.length;
  }
  varint(v) {
    if (typeof v === "number") {
      while (v >= 128) {
        this.byte(v % 128 | 128);
        v = Math.floor(v / 128);
      }
      this.byte(v);
    } else {
      while (v >= 0x80n) {
        this.byte(Number(v & 0x7fn) | 128);
        v >>= 7n;
      }
      this.byte(Number(v));
    }
  }
  out() {
    return this.buf.slice(0, this.len);
  }
};
var R = class {
  i = 0;
  b;
  constructor(b) {
    this.b = b;
  }
  take(n) {
    if (n < 0 || this.i + n > this.b.length) throw new DecodeError("unexpected end of data");
    const v = this.b.subarray(this.i, this.i + n);
    this.i += n;
    return v;
  }
  byte() {
    return this.take(1)[0];
  }
  varint() {
    let x = 0, mul = 1;
    for (let n = 0; n < 8; n++) {
      const c = this.byte();
      x += (c & 127) * mul;
      if (!(c & 128)) return x;
      mul *= 128;
    }
    throw new DecodeError("varint too long for a number");
  }
  varbig() {
    let x = 0n, sh = 0n;
    for (let n = 0; n < 20; n++) {
      const c = this.byte();
      x |= BigInt(c & 127) << sh;
      if (!(c & 128)) return x;
      sh += 7n;
    }
    throw new DecodeError("varint too long");
  }
  dv(n) {
    const s = this.take(n);
    return new DataView(s.buffer, s.byteOffset, n);
  }
};
var te = new TextEncoder();
var td = new TextDecoder("utf-8", { fatal: true });
function uint(bits) {
  const mx = 2 ** bits - 1;
  return {
    enc(w, v) {
      if (!Number.isInteger(v) || v < 0 || v > mx) throw new RangeError(`u${bits} out of range: ${v}`);
      w.varint(v);
    },
    dec(r) {
      const v = r.varint();
      if (v > mx) throw new DecodeError(`u${bits} overflow`);
      return v;
    }
  };
}
function sint(bits) {
  const lo = -(2 ** (bits - 1)), hi = 2 ** (bits - 1) - 1;
  return {
    enc(w, v) {
      if (!Number.isInteger(v) || v < lo || v > hi) throw new RangeError(`i${bits} out of range: ${v}`);
      w.varint(v >= 0 ? v * 2 : -v * 2 - 1);
    },
    dec(r) {
      const z = r.varint();
      const v = z % 2 === 0 ? z / 2 : -(z + 1) / 2;
      if (v < lo || v > hi) throw new DecodeError(`i${bits} overflow`);
      return v;
    }
  };
}
function ubig(bits) {
  const mx = (1n << BigInt(bits)) - 1n;
  return {
    enc(w, v) {
      if (typeof v !== "bigint" || v < 0n || v > mx) throw new RangeError(`u${bits} out of range: ${v}`);
      w.varint(v);
    },
    dec(r) {
      const v = r.varbig();
      if (v > mx) throw new DecodeError(`u${bits} overflow`);
      return v;
    }
  };
}
function sbig(bits) {
  const b = BigInt(bits);
  const lo = -(1n << b - 1n), hi = (1n << b - 1n) - 1n;
  return {
    enc(w, v) {
      if (typeof v !== "bigint" || v < lo || v > hi) throw new RangeError(`i${bits} out of range: ${v}`);
      w.varint(v << 1n ^ v >> b - 1n);
    },
    dec(r) {
      const z = r.varbig();
      const v = z >> 1n ^ -(z & 1n);
      if (v < lo || v > hi) throw new DecodeError(`i${bits} overflow`);
      return v;
    }
  };
}
var u8 = {
  enc(w, v) {
    if (!Number.isInteger(v) || v < 0 || v > 255) throw new RangeError(`u8 out of range: ${v}`);
    w.byte(v);
  },
  dec: (r) => r.byte()
};
var i8 = {
  enc(w, v) {
    if (!Number.isInteger(v) || v < -128 || v > 127) throw new RangeError(`i8 out of range: ${v}`);
    w.byte(v & 255);
  },
  dec: (r) => r.byte() << 24 >> 24
};
var u16 = uint(16);
var u32 = uint(32);
var i16 = sint(16);
var i32 = sint(32);
var u64 = ubig(64);
var u128 = ubig(128);
var i64 = sbig(64);
var i128 = sbig(128);
var f32 = {
  enc(w, v) {
    const d = new DataView(new ArrayBuffer(4));
    d.setFloat32(0, v, true);
    w.bytes(new Uint8Array(d.buffer));
  },
  dec: (r) => r.dv(4).getFloat32(0, true)
};
var f64 = {
  enc(w, v) {
    const d = new DataView(new ArrayBuffer(8));
    d.setFloat64(0, v, true);
    w.bytes(new Uint8Array(d.buffer));
  },
  dec: (r) => r.dv(8).getFloat64(0, true)
};
var bool = {
  enc: (w, v) => w.byte(v ? 1 : 0),
  dec(r) {
    const c = r.byte();
    if (c > 1) throw new DecodeError("bad bool");
    return c === 1;
  }
};
var str = {
  enc(w, v) {
    const d = te.encode(v);
    w.varint(d.length);
    w.bytes(d);
  },
  dec(r) {
    try {
      return td.decode(r.take(r.varint()));
    } catch (e) {
      if (e instanceof WireError) throw e;
      throw new DecodeError(String(e));
    }
  }
};
var unit = { enc() {
}, dec: () => null };
var bytes = {
  enc(w, v) {
    w.varint(v.length);
    w.bytes(v);
  },
  dec: (r) => r.take(r.varint()).slice()
};
function fixedBytes(n) {
  return {
    enc(w, v) {
      if (v.length !== n) throw new RangeError(`expected ${n} bytes, got ${v.length}`);
      w.bytes(v);
    },
    dec: (r) => r.take(n).slice()
  };
}
function vec(c) {
  return {
    enc(w, v) {
      w.varint(v.length);
      for (const x of v) c.enc(w, x);
    },
    dec(r) {
      const n = r.varint();
      if (n > MAX_PAYLOAD) throw new DecodeError("vec too long");
      const o = [];
      for (let i = 0; i < n; i++) o.push(c.dec(r));
      return o;
    }
  };
}
function opt(c) {
  return {
    enc(w, v) {
      if (v === null || v === void 0) w.byte(0);
      else {
        w.byte(1);
        c.enc(w, v);
      }
    },
    dec(r) {
      const t = r.byte();
      if (t === 0) return null;
      if (t === 1) return c.dec(r);
      throw new DecodeError("bad option tag");
    }
  };
}
function res(ok, err) {
  return {
    enc(w, v) {
      if ("ok" in v) {
        w.byte(0);
        ok.enc(w, v.ok);
      } else {
        w.byte(1);
        err.enc(w, v.err);
      }
    },
    dec(r) {
      const t = r.byte();
      if (t === 0) return { ok: ok.dec(r) };
      if (t === 1) return { err: err.dec(r) };
      throw new DecodeError("bad result tag");
    }
  };
}
function tup(...cs) {
  return {
    enc(w, v) {
      if (v.length !== cs.length) throw new RangeError(`expected a ${cs.length}-tuple`);
      cs.forEach((c, i) => c.enc(w, v[i]));
    },
    dec: (r) => cs.map((c) => c.dec(r))
  };
}
function arr(n, c) {
  return {
    enc(w, v) {
      if (v.length !== n) throw new RangeError(`expected ${n} items, got ${v.length}`);
      for (const x of v) c.enc(w, x);
    },
    dec(r) {
      const o = [];
      for (let i = 0; i < n; i++) o.push(c.dec(r));
      return o;
    }
  };
}
var TABLE = {};
function ref(name) {
  return { enc: (w, v) => TABLE[name].enc(w, v), dec: (r) => TABLE[name].dec(r) };
}
function struct(fields) {
  return {
    enc(w, v) {
      for (const [k, c] of fields) c.enc(w, v[k]);
    },
    dec(r) {
      const o = {};
      for (const [k, c] of fields) o[k] = c.dec(r);
      return o;
    }
  };
}
function enumOf(variants) {
  return {
    enc(w, v) {
      const i = variants.findIndex(([n]) => n === v.tag);
      if (i < 0) throw new RangeError(`not a variant of this enum: ${JSON.stringify(v)}`);
      w.varint(i);
      for (const [k, c] of variants[i][1]) c.enc(w, v[k]);
    },
    dec(r) {
      const i = r.varint();
      if (i >= variants.length) throw new DecodeError(`enum variant ${i} out of range`);
      const o = { tag: variants[i][0] };
      for (const [k, c] of variants[i][1]) o[k] = c.dec(r);
      return o;
    }
  };
}
function encode(c, v) {
  const w = new W();
  c.enc(w, v);
  return w.out();
}
function decode(c, body) {
  const r = new R(body);
  const v = c.dec(r);
  if (r.i !== body.length) throw new DecodeError("trailing bytes");
  return v;
}
function frame(kind, id, op, payload = new Uint8Array()) {
  const b = new Uint8Array(HEADER + payload.length);
  const d = new DataView(b.buffer);
  d.setUint32(0, payload.length, true);
  b[4] = kind;
  d.setBigUint64(5, BigInt(id), true);
  d.setUint32(13, op, true);
  b.set(payload, HEADER);
  return b;
}
function adaptError(r) {
  const i = r.varint();
  switch (i) {
    case 0:
      return { kind: "Narrow", field: str.dec(r), value: str.dec(r) };
    case 1:
      return { kind: "NoVariant", field: str.dec(r), variant: str.dec(r) };
    case 2:
      return { kind: "Missing", field: str.dec(r) };
    case 3:
      return { kind: "NoPath" };
    case 4:
      return { kind: "Decode", why: str.dec(r) };
    default:
      throw new DecodeError(`unknown AdaptError variant ${i}`);
  }
}
function refusal(body) {
  const r = new R(body);
  const i = r.varint();
  switch (i) {
    case 0:
      return { ok: r.varbig() };
    case 1: {
      const ours = r.varbig(), theirs = r.varbig(), api = str.dec(r);
      return new Refused("Schema", `server has schema 0x${ours.toString(16)}, this client 0x${theirs.toString(16)} (api ${api})`, { ours, theirs, api });
    }
    case 4:
      return new Refused("UnknownOp", r.varint());
    case 8: {
      const ours = r.varbig(), theirs = r.varbig(), api = str.dec(r), update = str.dec(r);
      return new Refused("SchemaAt", `server has schema 0x${ours.toString(16)}, this client 0x${theirs.toString(16)} (api ${api}): ${update}`, { ours, theirs, api, update });
    }
    case 9: {
      const api = str.dec(r), op = r.varint(), op_name = str.dec(r), ours = r.varint(), theirs = r.varint();
      const why = adaptError(r);
      return new Refused("Adapt", `${api} op ${op_name} (#${op}) v${theirs} <-> v${ours}: ${JSON.stringify(why)}`, { api, op, op_name, ours, theirs, why });
    }
    case 10: {
      const api = str.dec(r), op = r.varint(), op_name = str.dec(r), ours = r.varbig(), theirs = r.varbig();
      return new Refused("SchemaMismatch", `${api} op ${op_name} (#${op}): server shape 0x${ours.toString(16)}, this client 0x${theirs.toString(16)}`, { api, op, op_name, ours, theirs });
    }
    default: {
      const names = { 2: "Gate", 3: "Right", 5: "NoCell", 6: "Decode", 7: "Remote" };
      if (!(i in names)) throw new DecodeError(`unknown Refusal variant ${i}`);
      return new Refused(names[i], str.dec(r));
    }
  }
}
function refused(body) {
  try {
    const r = refusal(body);
    return r instanceof Refused ? r : new Refused("Ok");
  } catch {
    return new Refused("Remote", "bad error body");
  }
}
var HELLO_DOMAIN = "ce-wire-hello-v1";
var PKCS8_ED25519 = Uint8Array.from([48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 112, 4, 34, 4, 32]);
function b64u(s) {
  const t = atob(s.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - s.length % 4) % 4));
  return Uint8Array.from(t, (c) => c.charCodeAt(0));
}
function everyoneGrant(url, prefix) {
  return { url, seed: new Uint8Array(0), chain: new Uint8Array(0), everyone: prefix };
}
function grantFromLink(href = location.href) {
  const u = new URL(href);
  const f = new URLSearchParams(u.hash.slice(1));
  const g = f.get("g");
  if (!g) return null;
  const [k, c] = g.split(".");
  const url = f.get("w") ?? `${u.protocol === "https:" ? "wss" : "ws"}://${u.host}/wire`;
  return { url, seed: b64u(k), chain: b64u(c) };
}
var WebKey = class _WebKey {
  nodeId;
  key;
  constructor(nodeId, key) {
    this.nodeId = nodeId;
    this.key = key;
  }
  static async fromSeed(seed) {
    const der = new Uint8Array(48);
    der.set(PKCS8_ED25519);
    der.set(seed, 16);
    const key = await crypto.subtle.importKey("pkcs8", der, { name: "Ed25519" }, true, ["sign"]);
    const jwk = await crypto.subtle.exportKey("jwk", key);
    return new _WebKey(b64u(jwk.x), key);
  }
  async sign(data) {
    return new Uint8Array(await crypto.subtle.sign({ name: "Ed25519" }, this.key, data));
  }
};
async function webProof(g, api = API_NAME, schema = SCHEMA) {
  const k = await WebKey.fromSeed(g.seed);
  const unixMs = BigInt(Date.now());
  const nonce = crypto.getRandomValues(new Uint8Array(16));
  const s = new W();
  str.enc(s, HELLO_DOMAIN);
  str.enc(s, api);
  u64.enc(s, schema);
  u64.enc(s, unixMs);
  s.bytes(nonce);
  s.bytes(k.nodeId);
  bytes.enc(s, g.chain);
  const sig = await k.sign(s.out());
  const p = new W();
  p.bytes(k.nodeId);
  u64.enc(p, unixMs);
  p.bytes(nonce);
  bytes.enc(p, g.chain);
  bytes.enc(p, sig);
  return p.out();
}
var Subscription = class {
  q = [];
  waiter = null;
  done = false;
  c;
  id;
  op;
  codec;
  constructor(c, id, op, codec) {
    this.c = c;
    this.id = id;
    this.op = op;
    this.codec = codec;
  }
  /** @internal */
  push(m) {
    if (this.waiter) {
      const w = this.waiter;
      this.waiter = null;
      w(m);
    } else this.q.push(m);
  }
  /** Next item, or `undefined` when the server ended the stream normally. */
  async next(timeoutMs) {
    if (this.done) return void 0;
    const m = await new Promise((res2, rej) => {
      if (this.q.length) return res2(this.q.shift());
      let t;
      this.waiter = (x) => {
        if (t) clearTimeout(t);
        res2(x);
      };
      if (timeoutMs !== void 0)
        t = setTimeout(() => {
          this.waiter = null;
          rej(new Timeout("no item"));
        }, timeoutMs);
    });
    if (m === null) {
      this.finish();
      throw new Closed("connection closed");
    }
    const [kind, body] = m;
    if (kind === K_ITEM) return decode(this.codec, body);
    this.finish();
    if (kind === K_END) return void 0;
    throw refused(body);
  }
  finish() {
    this.done = true;
    this.c.subs.delete(this.id);
  }
  close() {
    if (this.done) return;
    this.finish();
    this.c.send(frame(K_END, this.id, this.op));
  }
  async *[Symbol.asyncIterator]() {
    try {
      for (; ; ) {
        const v = await this.next();
        if (v === void 0) return;
        yield v;
      }
    } finally {
      this.close();
    }
  }
};
var ConnBase = class _ConnBase {
  session = 0n;
  deadlineMs = CALL_DEADLINE_MS;
  nextId = 1;
  buf = new Uint8Array(0);
  pending = /* @__PURE__ */ new Map();
  /** @internal */
  subs = /* @__PURE__ */ new Map();
  closed = false;
  first = null;
  ws;
  constructor(ws) {
    this.ws = ws;
    ws.binaryType = "arraybuffer";
    ws.onmessage = (e) => this.onData(new Uint8Array(e.data));
    ws.onclose = () => this.onClose();
  }
  onData(d) {
    if (this.buf.length) {
      const b = new Uint8Array(this.buf.length + d.length);
      b.set(this.buf);
      b.set(d, this.buf.length);
      this.buf = b;
    } else this.buf = d;
    for (; ; ) {
      if (this.buf.length < HEADER) return;
      const v = new DataView(this.buf.buffer, this.buf.byteOffset, this.buf.length);
      const len = v.getUint32(0, true);
      if (len > MAX_PAYLOAD) return void this.ws.close();
      if (this.buf.length < HEADER + len) return;
      const kind = this.buf[4];
      const id = Number(v.getBigUint64(5, true));
      const body = this.buf.slice(HEADER, HEADER + len);
      this.buf = this.buf.subarray(HEADER + len);
      if (this.first) {
        const f = this.first;
        this.first = null;
        f([kind, id, body]);
        continue;
      }
      const sub = kind === K_ITEM || kind === K_END || kind === K_ERR ? this.subs.get(id) : void 0;
      if (sub) {
        sub.push([kind, body]);
        if (kind !== K_ITEM) this.subs.delete(id);
        continue;
      }
      const w = this.pending.get(id);
      if (w) {
        this.pending.delete(id);
        w([kind, body]);
      }
    }
  }
  onClose() {
    this.closed = true;
    if (this.first) {
      const f = this.first;
      this.first = null;
      f(null);
    }
    for (const w of this.pending.values()) w(null);
    this.pending.clear();
    for (const s of this.subs.values()) s.push(null);
    this.subs.clear();
  }
  /** @internal */
  send(b) {
    if (this.closed) throw new Closed("connection closed");
    this.ws.send(b);
  }
  /** @internal Open `url`, say Hello as `api`, wait for the answer. */
  static async open(url, hello, waitMs) {
    const ws = new WebSocket(url);
    await new Promise((res2, rej) => {
      const t = setTimeout(() => {
        ws.close();
        rej(new Timeout(`connect ${url}`));
      }, waitMs);
      ws.onopen = () => {
        clearTimeout(t);
        res2();
      };
      ws.onerror = () => {
        clearTimeout(t);
        rej(new WireError(`connect ${url}: refused or unreachable`));
      };
    });
    const c = new _ConnBase(ws);
    const ans = new Promise((res2, rej) => {
      c.first = res2;
      setTimeout(() => {
        if (c.first === res2) {
          c.first = null;
          ws.close();
          rej(new Timeout("no Hello answer"));
        }
      }, waitMs);
    });
    ws.send(frame(K_HELLO, 0, 0, hello));
    const m = await ans;
    if (m === null) throw new Closed("connection closed");
    const r = refusal(m[2]);
    if (m[0] !== K_HELLO || r instanceof Refused) {
      ws.close();
      throw r instanceof Refused ? r : new Refused("Ok");
    }
    c.session = r.ok;
    return c;
  }
  /** @internal */
  async call(op, qc, pc, req, waitMs = 0) {
    const deadlineMs = Math.max(this.deadlineMs, waitMs);
    const payload = encode(qc, req);
    const id = this.nextId++;
    const got = await new Promise((res2, rej) => {
      const t = setTimeout(() => {
        this.pending.delete(id);
        rej(new Timeout(`op ${op} took over ${deadlineMs} ms`));
      }, deadlineMs);
      this.pending.set(id, (m) => {
        clearTimeout(t);
        res2(m);
      });
      try {
        this.send(frame(K_REQ, id, op, payload));
      } catch (e) {
        clearTimeout(t);
        this.pending.delete(id);
        rej(e);
      }
    });
    if (got === null) throw new Closed("connection closed");
    if (got[0] === K_RESP) return decode(pc, got[1]);
    throw refused(got[1]);
  }
  /** @internal */
  subscribeOp(op, qc, ic, req) {
    const payload = encode(qc, req);
    const id = this.nextId++;
    const sub = new Subscription(this, id, op, ic);
    if (this.closed) throw new Closed("connection closed");
    this.subs.set(id, sub);
    this.send(frame(K_SUB, id, op, payload));
    return sub;
  }
  close() {
    this.ws.close();
  }
};
async function connectWeb(g, waitMs, instance = "") {
  const key = instance ? `${API_NAME}#${instance}` : API_NAME;
  const w = new W();
  str.enc(w, key);
  u32.enc(w, API_VERSION);
  u64.enc(w, SCHEMA);
  bytes.enc(w, g.everyone !== void 0 ? new TextEncoder().encode("ceeveryn" + g.everyone) : await webProof(g, key));
  bool.enc(w, false);
  return ConnBase.open(g.url, w.out(), waitMs);
}
function linkLine(s) {
  switch (s.state) {
    case "finding":
      return "finding the cell";
    case "stale":
      return `live, nothing for ${Math.floor(s.quiet_ms / 1e3)} s`;
    case "reconnecting":
      return `reconnecting in ${Math.ceil(s.wait_ms / 1e3)} s (try ${s.attempt}): ${s.why}`;
    case "refused":
    case "failed":
      return `${s.state}: ${s.why}`;
    default:
      return s.state;
  }
}
function retryable(e) {
  return !(e instanceof Refused && ["Gate", "Right", "Schema", "UnknownOp"].includes(e.kind));
}
function linkWait(attempt, firstWaitMs = 1e3, maxWaitMs = 2e4) {
  return Math.min(firstWaitMs * 2 ** Math.min(Math.max(attempt - 1, 0), 20), maxWaitMs);
}
var Link = class {
  state = { state: "idle" };
  cfg;
  stopped = false;
  wake = null;
  connectFn;
  onState;
  constructor(connectFn, onState = () => {
  }, cfg = {}) {
    this.connectFn = connectFn;
    this.onState = onState;
    this.cfg = { firstWaitMs: 1e3, maxWaitMs: 2e4, maxAttempts: 0, staleAfterMs: 3e4, ...cfg };
  }
  set(s) {
    this.state = s;
    this.onState(s);
  }
  cur = null;
  /** End the link now: a follow waiting on its stream returns at once (the open client is closed). */
  stop() {
    this.stopped = true;
    this.wake?.();
    try {
      this.cur?.close();
    } catch {
    }
  }
  sleep(ms) {
    return new Promise((res2) => {
      const t = setTimeout(() => (this.wake = null, res2()), ms);
      this.wake = () => (clearTimeout(t), this.wake = null, res2());
    });
  }
  /** -> the next attempt number, or null when the link ended (refused, failed, stopped). */
  async afterError(attempt, e) {
    const why = e instanceof Error ? e.message : String(e);
    if (!retryable(e)) {
      this.set({ state: "refused", why });
      return null;
    }
    attempt += 1;
    if (this.cfg.maxAttempts && attempt >= this.cfg.maxAttempts) {
      this.set({ state: "failed", why: `${attempt} tries: ${why}` });
      return null;
    }
    const wait_ms = linkWait(attempt, this.cfg.firstWaitMs, this.cfg.maxWaitMs);
    this.set({ state: "reconnecting", attempt, wait_ms, why });
    await this.sleep(wait_ms);
    return this.stopped ? null : attempt;
  }
  /** A connected client (state live), or null (state refused / failed / stopped). */
  async connect() {
    let attempt = 0;
    while (!this.stopped) {
      this.set({ state: "finding" });
      try {
        const c = await this.connectFn();
        this.set({ state: "live", since_ms: Date.now() });
        return c;
      } catch (e) {
        attempt = await this.afterError(attempt, e);
        if (attempt === null) return null;
      }
    }
    return null;
  }
  /** Follow a stream until refused, failed or stop(): `openSub(client)` opens it (called on every reconnect, so a
   *  cursor can resume), `onItem` gets each item. -> the final state. */
  async follow(openSub, onItem) {
    let attempt = 0;
    while (!this.stopped) {
      const c = await this.connect();
      if (!c) return this.state;
      this.cur = c;
      if (this.stopped) c.close();
      let err = new Closed("stream ended");
      try {
        const sub = openSub(c);
        let quiet = 0;
        while (!this.stopped) {
          let item;
          try {
            item = await sub.next(this.cfg.staleAfterMs);
          } catch (e) {
            if (e instanceof Timeout) {
              quiet += this.cfg.staleAfterMs;
              this.set({ state: "stale", quiet_ms: quiet });
              continue;
            }
            throw e;
          }
          if (item === void 0) break;
          if (this.state.state !== "live") this.set({ state: "live", since_ms: Date.now() });
          quiet = 0;
          attempt = 0;
          onItem(item);
        }
      } catch (e) {
        err = e;
      } finally {
        this.cur = null;
        try {
          c.close();
        } catch {
        }
      }
      if (this.stopped) break;
      attempt = await this.afterError(attempt, err);
      if (attempt === null) return this.state;
    }
    this.set({ state: "idle" });
    return this.state;
  }
};
TABLE["Health"] = struct([["ok", bool], ["version", str], ["uptime_s", u64]]);
TABLE["RunReq"] = struct([["args", vec(str)], ["input", bytes], ["in_ext", str], ["out_ext", str]]);
TABLE["ToolId"] = struct([["name", str], ["version", str], ["artifact_sha256", str], ["os_arch", str], ["node", str]]);
TABLE["RunResp"] = struct([["exit", i32], ["signal", str], ["stdout", bytes], ["stderr", bytes], ["stdout_sha256", str], ["stderr_sha256", str], ["output", bytes], ["output_cid", str], ["argv", vec(str)], ["tool", ref("ToolId")], ["ms", u64]]);
TABLE["Flag"] = struct([["name", str], ["value", str]]);
TABLE["Policy"] = struct([["api", str], ["license", str], ["prefix", vec(str)], ["flags", vec(ref("Flag"))], ["positional", str], ["in_ext", vec(str)], ["out_ext", vec(str)], ["max_in_bytes", u64], ["max_out_bytes", u64], ["max_stdio_bytes", u64], ["timeout_ms", u64], ["mem_mb", u64], ["concurrency", u32], ["tool", ref("ToolId")]]);
TABLE["RunError"] = enumOf([["Invalid", [["f0", str]]], ["Limited", [["f0", str]]], ["Failed", [["f0", str]]]]);
var RIGHTS = {
  "healthOp": "wrap.convert:health",
  "run": "wrap.convert:run",
  "describe": "wrap.convert:describe"
};
var WrapConvertClient = class _WrapConvertClient {
  conn;
  constructor(conn) {
    this.conn = conn;
  }
  /** Through the wire.ws-door in `g` (default: the grant in this page's link); `instance` names one instance of the api (Rust by_instance). */
  static async connect(g = grantFromLink(), waitMs = 1e4, instance = "") {
    if (!g) throw new Refused("Gate", "no grant in this link (#g=...)");
    return new _WrapConvertClient(await connectWeb(g, waitMs, instance));
  }
  close() {
    this.conn.close();
  }
  /** op HealthOp (needs wrap.convert:health). */
  healthOp() {
    return this.conn.call(1, unit, ref("Health"), null);
  }
  /** op Run (needs wrap.convert:run). */
  run(req) {
    return this.conn.call(2, ref("RunReq"), res(ref("RunResp"), ref("RunError")), req, 3e4);
  }
  /** op Describe (needs wrap.convert:describe). */
  describe() {
    return this.conn.call(3, unit, ref("Policy"), null);
  }
};
export {
  API_NAME,
  API_VERSION,
  CALL_DEADLINE_MS,
  Closed,
  ConnBase,
  DEADLINE_MS,
  DOOR_PORT,
  DecodeError,
  Link,
  NETFIND_SCHEMA,
  RIGHTS,
  Refused,
  SCHEMA,
  SPAWN_WAIT_MS,
  Subscription,
  Timeout,
  Unreachable,
  WireError,
  WrapConvertClient,
  arr,
  bool,
  bytes,
  everyoneGrant,
  f32,
  f64,
  fixedBytes,
  grantFromLink,
  i128,
  i16,
  i32,
  i64,
  i8,
  linkLine,
  linkWait,
  opt,
  res,
  retryable,
  str,
  tup,
  u128,
  u16,
  u32,
  u64,
  u8,
  unit,
  vec,
  webProof
};
