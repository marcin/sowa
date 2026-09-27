// Runtime Sowy dla Buna. Kompilator dokleja pod nim kod programu.
// Wartości: Int to liczba JS, Money to Dec (bigint ze skalą 20), String, Bool, Html, Date, DateTime,
// lista to tablica, rekord to {$r: "Nazwa", ...pola}, wariant to {$v: "Nazwa", ...pola}.
import { Database } from "bun:sqlite";

// ---------- błędy ----------

// return z bloku `or` i z `try`: łapie go najbliższa funkcja albo lambda.
class $Ret {
  constructor(v) { this.v = v; }
}
// Wartość niezgodna z typem: błąd programu.
class $TypeErr extends Error {}
// Niespełniony przykład w teście.
class $Fail extends Error {}
const $FAIL = Symbol("fail");

// ---------- liczby ----------

const $SC = 20n;
const $S = 10n ** $SC;

class Dec {
  constructor(m) { this.m = m; }
}

function $dec(s) {
  const t = String(s).trim();
  const m = /^(-)?(\d+)(?:\.(\d+))?$/.exec(t);
  if (!m) throw new $TypeErr(`to nie jest kwota: ${JSON.stringify(s)}`);
  const frac = (m[3] || "").slice(0, Number($SC)).padEnd(Number($SC), "0");
  const v = BigInt(m[2]) * $S + BigInt(frac || "0");
  return new Dec(m[1] ? -v : v);
}

function $isnum(x) { return typeof x === "number" || x instanceof Dec; }
function $D(x) {
  if (x instanceof Dec) return x;
  if (typeof x === "number") return new Dec(BigInt(x) * $S);
  throw new $TypeErr(`oczekiwano liczby, jest ${$show(x)}`);
}
function $int(n) {
  if (!Number.isSafeInteger(n)) throw new $TypeErr(`liczba poza zakresem Int: ${n}`);
  return n;
}

// Zaokrąglenie połówek od zera do `places` miejsc.
function $round_dec(d, places) {
  const unit = 10n ** ($SC - BigInt(places));
  const neg = d.m < 0n;
  const a = neg ? -d.m : d.m;
  let q = a / unit;
  if ((a % unit) * 2n >= unit) q += 1n;
  const r = q * unit;
  return new Dec(neg ? -r : r);
}

function $dec_text(d, min_places, max_places) {
  let x = d;
  if (max_places !== undefined) x = $round_dec(d, max_places);
  const neg = x.m < 0n;
  const a = neg ? -x.m : x.m;
  const ip = a / $S;
  let fp = (a % $S).toString().padStart(Number($SC), "0");
  if (max_places !== undefined) fp = fp.slice(0, max_places);
  fp = fp.replace(/0+$/, "");
  while (fp.length < min_places) fp += "0";
  const s = ip.toString() + (fp ? "." + fp : "");
  return (neg && /[1-9]/.test(s) ? "-" : "") + s;
}

function $add(a, b) {
  if (typeof a === "string" && typeof b === "string") return a + b;
  if (Array.isArray(a) && Array.isArray(b)) return [...a, ...b];
  if (typeof a === "number" && typeof b === "number") return $int(a + b);
  if ($isnum(a) && $isnum(b)) return new Dec($D(a).m + $D(b).m);
  throw new $TypeErr(`nie da się dodać ${$show(a)} i ${$show(b)}`);
}
function $sub(a, b) {
  if (typeof a === "number" && typeof b === "number") return $int(a - b);
  if ($isnum(a) && $isnum(b)) return new Dec($D(a).m - $D(b).m);
  throw new $TypeErr(`nie da się odjąć ${$show(b)} od ${$show(a)}`);
}
function $mul(a, b) {
  if (typeof a === "number" && typeof b === "number") return $int(a * b);
  if ($isnum(a) && $isnum(b)) return new Dec(($D(a).m * $D(b).m) / $S);
  throw new $TypeErr(`nie da się pomnożyć ${$show(a)} i ${$show(b)}`);
}
function $div(a, b) {
  if (typeof a === "number" && typeof b === "number") {
    if (b === 0) throw new $TypeErr("dzielenie przez zero");
    return Math.trunc(a / b);
  }
  if ($isnum(a) && $isnum(b)) {
    const d = $D(b).m;
    if (d === 0n) throw new $TypeErr("dzielenie przez zero");
    return new Dec(($D(a).m * $S) / d);
  }
  throw new $TypeErr(`nie da się podzielić ${$show(a)} przez ${$show(b)}`);
}
function $mod(a, b) {
  if (typeof a === "number" && typeof b === "number" && b !== 0) return a % b;
  throw new $TypeErr(`reszta z dzielenia tylko dla Int: ${$show(a)} % ${$show(b)}`);
}
function $neg(a) {
  if (typeof a === "number") return -a;
  if (a instanceof Dec) return new Dec(-a.m);
  throw new $TypeErr(`nie da się zanegować ${$show(a)}`);
}

// ---------- daty ----------

class $Date {
  constructor(y, m, d) { this.y = y; this.m = m; this.d = d; }
  key() { return this.y * 10000 + this.m * 100 + this.d; }
}
class $DateTime {
  constructor(y, mo, d, h, mi, s) { Object.assign(this, { y, mo, d, h, mi, s }); }
  key() { return ((((this.y * 100 + this.mo) * 100 + this.d) * 100 + this.h) * 100 + this.mi) * 100 + this.s; }
}
const $p2 = (n) => String(n).padStart(2, "0");
function $date_text(d) { return `${String(d.y).padStart(4, "0")}-${$p2(d.m)}-${$p2(d.d)}`; }
function $dt_text(t, sep) { return `${String(t.y).padStart(4, "0")}-${$p2(t.mo)}-${$p2(t.d)}${sep}${$p2(t.h)}:${$p2(t.mi)}:${$p2(t.s)}`; }
function $valid_date(y, m, d) {
  if (m < 1 || m > 12 || d < 1) return false;
  return d <= new Date(Date.UTC(y, m, 0)).getUTCDate();
}
function $parse_date(s) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m || !$valid_date(+m[1], +m[2], +m[3])) throw new $TypeErr(`to nie jest data: ${JSON.stringify(s)}`);
  return new $Date(+m[1], +m[2], +m[3]);
}
function $parse_dt(s) {
  const m = /^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2}))?$/.exec(s);
  if (!m || !$valid_date(+m[1], +m[2], +m[3]) || +m[4] > 23 || +m[5] > 59 || +(m[6] || 0) > 59)
    throw new $TypeErr(`to nie jest data z godziną: ${JSON.stringify(s)}`);
  return new $DateTime(+m[1], +m[2], +m[3], +m[4], +m[5], +(m[6] || 0));
}

// ---------- Html ----------

class $Html {
  constructor(s) { this.s = s; }
}
function $raw(s) { return new $Html(s); }
function $esc(s) {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}
function $html_part(x) {
  if (x instanceof $Html) return x.s;
  if (Array.isArray(x)) return x.map($html_part).join("");
  return $esc($disp(x));
}
function $html(parts) { return new $Html(parts.map($html_part).join("")); }

// ---------- wyświetlanie ----------

// to_string: tekst dla człowieka.
function $disp(x) {
  if (typeof x === "string") return x;
  if (typeof x === "number" || typeof x === "boolean") return String(x);
  if (x instanceof Dec) return $dec_text(x, 2, 2);
  if (x instanceof $Date) return $date_text(x);
  if (x instanceof $DateTime) return $dt_text(x, " ");
  if (x instanceof $Html) return x.s;
  if (x && x.$v && Object.keys(x).length === 1) return x.$v;
  return $show(x);
}

// Zapis wartości w komunikatach testów: jak w kodzie Sowy.
function $show(x) {
  if (x === undefined) return "(brak wartości)";
  if (x === null) return "null";
  if (typeof x === "string") return JSON.stringify(x);
  if (typeof x === "number" || typeof x === "boolean") return String(x);
  if (x instanceof Dec) return $dec_text(x, 2);
  if (x instanceof $Date) return $date_text(x);
  if (x instanceof $DateTime) return $dt_text(x, "T");
  if (x instanceof $Html) return `html${JSON.stringify(x.s)}`;
  if (Array.isArray(x)) return `[${x.map($show).join(", ")}]`;
  if (typeof x === "function") return "(funkcja)";
  if (x.$cap) return `(${x.$cap})`;
  const name = x.$r || x.$v;
  if (name) {
    const fs = Object.keys(x).filter((k) => k[0] !== "$");
    if (fs.length === 0) return name;
    return `${name}(${fs.map((k) => `${k}: ${$show(x[k])}`).join(", ")})`;
  }
  return String(x);
}

// ---------- porównania ----------

function $eq(a, b) {
  if (a === b) return true;
  if ($isnum(a) && $isnum(b)) return $D(a).m === $D(b).m;
  if (a instanceof $Date && b instanceof $Date) return a.key() === b.key();
  if (a instanceof $DateTime && b instanceof $DateTime) return a.key() === b.key();
  if (a instanceof $Html && b instanceof $Html) return a.s === b.s;
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((x, i) => $eq(x, b[i]));
  if (a && b && typeof a === "object" && typeof b === "object" && (a.$r || a.$v)) {
    if (a.$r !== b.$r || a.$v !== b.$v) return false;
    const ka = Object.keys(a), kb = Object.keys(b);
    return ka.length === kb.length && ka.every((k) => $eq(a[k], b[k]));
  }
  return false;
}

function $cmp(a, b) {
  if (typeof a === "number" && typeof b === "number") return a < b ? -1 : a > b ? 1 : 0;
  if ($isnum(a) && $isnum(b)) {
    const x = $D(a).m, y = $D(b).m;
    return x < y ? -1 : x > y ? 1 : 0;
  }
  if (typeof a === "string" && typeof b === "string") return a < b ? -1 : a > b ? 1 : 0;
  if ((a instanceof $Date && b instanceof $Date) || (a instanceof $DateTime && b instanceof $DateTime)) {
    return Math.sign(a.key() - b.key());
  }
  throw new $TypeErr(`nie da się porównać ${$show(a)} i ${$show(b)}`);
}

function $bool(x) {
  if (typeof x !== "boolean") throw new $TypeErr(`oczekiwano Bool, jest ${$show(x)}`);
  return x;
}
function $iter(x) {
  if (!Array.isArray(x)) throw new $TypeErr(`for działa na liście, a dostał ${$show(x)}`);
  return x;
}

// ---------- typy w runtime ----------

const $P = {
  Int: { k: "prim", n: "Int" },
  Money: { k: "prim", n: "Money" },
  String: { k: "prim", n: "String" },
  Bool: { k: "prim", n: "Bool" },
  Html: { k: "prim", n: "Html" },
  Date: { k: "prim", n: "Date" },
  DateTime: { k: "prim", n: "DateTime" },
  Any: { k: "any" },
};
const $list = (e) => ({ k: "list", e });
const $cap = (n) => ({ k: "cap", n });
const $ref = (n) => ({ k: "ref", n });
const $var = (n) => ({ k: "var", n });
const $union = (a) => ({ k: "union", a });
const $refine = (b, c, src, h) => ({ k: "refine", b, c, src, h });
const $rec = (n, f) => ({ k: "rec", n, f });
const $named = (n, d) => ({ k: "named", n, d });

const $T = {};
const $VD = {};
const $V = {};

$T.HttpRequest = $rec("HttpRequest", [["method", $ref("Method")], ["path", $P.String], ["body", $P.String]]);
$T.HttpResponse = $rec("HttpResponse", [["status", $P.Int], ["body", $P.String]]);
$T.Method = $named("Method", $union(["Get", "Post", "Put", "Patch", "Delete"].map($var)));
$T.MailError = $named("MailError", $union(["MailRejected", "MailTimeout"].map($var)));
for (const n of ["Get", "Post", "Put", "Patch", "Delete", "HttpError", "DbError", "NoRow", "NotANumber", "Sent", "MailRejected", "MailTimeout"]) $VD[n] = null;

function $init_variants() {
  for (const n of Object.keys($VD)) if ($VD[n] === null) $V[n] = Object.freeze({ $v: n });
}

function $lookup(n) {
  const d = $T[n];
  if (!d) throw new $TypeErr(`nieznany typ ${n}`);
  return d;
}

function $tname(d) {
  switch (d.k) {
    case "any": return "Any";
    case "prim": return d.n;
    case "list": return `List<${$tname(d.e)}>`;
    case "cap": return d.n;
    case "ref": case "var": case "rec": case "named": return d.n;
    case "union": return d.a.map($tname).join(" | ");
    case "refine": return d.src;
  }
  return "?";
}

function $prim_is(n, v) {
  switch (n) {
    case "Int": return Number.isInteger(v);
    case "Money": return v instanceof Dec || Number.isInteger(v);
    case "String": return typeof v === "string";
    case "Bool": return typeof v === "boolean";
    case "Html": return v instanceof $Html;
    case "Date": return v instanceof $Date;
    case "DateTime": return v instanceof $DateTime;
  }
  return false;
}

function $cap_ok(n, v) {
  return !!v && (v.$cap === n || (n === "DbRead" && v.$cap === "Db"));
}

// `is` i dopasowanie w match.
function $is(d, v) {
  switch (d.k) {
    case "any": return true;
    case "prim": return $prim_is(d.n, v);
    case "list": return Array.isArray(v) && v.every((x) => $is(d.e, x));
    case "cap": return $cap_ok(d.n, v);
    case "ref": return $is($lookup(d.n), v);
    case "named": return $is(d.d, v);
    case "rec": return !!v && v.$r === d.n;
    case "var": return !!v && v.$v === d.n;
    case "union": return d.a.some((a) => $is(a, v));
    case "refine": {
      if (!$is(d.b, v)) return false;
      try { return d.c($conform(d.b, v, "")) === true; } catch (e) { if (e instanceof $TypeErr) return false; throw e; }
    }
  }
  return false;
}

// Sprawdza wartość z typem i zwraca ją po zamianie Int na Money. Rzuca błąd programu.
function $conform(d, v, what) {
  const fail = (why) => {
    throw new $TypeErr(`${what ? what + ": " : ""}${$show(v)} nie jest ${$tname(d)}${why ? " (" + why + ")" : ""}`);
  };
  switch (d.k) {
    case "any": return v;
    case "prim":
      if (!$prim_is(d.n, v)) fail();
      if (d.n === "Money") return $D(v);
      return v;
    case "list":
      if (!Array.isArray(v)) fail();
      return v.map((x, i) => $conform(d.e, x, `${what}[${i}]`));
    case "cap": if (!$cap_ok(d.n, v)) fail(); return v;
    case "ref": return $conform($lookup(d.n), v, what);
    case "named":
      try { return $conform(d.d, v, what); } catch (e) { if (e instanceof $TypeErr) fail(); throw e; }
    case "rec": if (!v || v.$r !== d.n) fail(); return v;
    case "var": if (!v || v.$v !== d.n) fail(); return v;
    case "union":
      for (const a of d.a) {
        try { return $conform(a, v, what); } catch (e) { if (!(e instanceof $TypeErr)) throw e; }
      }
      fail();
    case "refine": {
      const x = $conform(d.b, v, what);
      if (d.c(x) !== true) fail("warunek");
      return x;
    }
  }
  fail();
}

function $as(d, v) {
  try {
    if (typeof v === "string") {
      const r = $target_rec(d);
      if (r) {
        const t = v.trimStart();
        const data = t.startsWith("{") || t.startsWith("[") ? $jparse(v) : $form_parse(v);
        return $conform(d, $from_json(r, data, "form"), "");
      }
    }
    return $conform(d, v, "");
  } catch (e) {
    if (e instanceof $TypeErr) return $FAIL;
    throw e;
  }
}
function $as_strict(d, v, src) {
  const r = $as(d, v);
  if (r === $FAIL) throw new $TypeErr(`${$show(v)} as ${src}: wartość nie pasuje do typu`);
  return r;
}
// Rekord, na który `as` zamienia tekst (przez zawężenia i nazwy typów).
function $target_rec(d) {
  for (let i = 0; i < 20; i++) {
    if (d.k === "ref") d = $lookup(d.n);
    else if (d.k === "named") d = d.d;
    else if (d.k === "refine") d = d.b;
    else break;
  }
  return d.k === "rec" ? d : null;
}

function $mk(n, fields) {
  const d = $lookup(n);
  const out = { $r: n };
  for (const [f, fd] of d.f) {
    if (!(f in fields)) throw new $TypeErr(`${n}: brak pola ${f}`);
    out[f] = $conform(fd, fields[f], `${n}.${f}`);
  }
  for (const k of Object.keys(fields)) if (!d.f.some(([f]) => f === k)) throw new $TypeErr(`${n} nie ma pola ${k}`);
  return out;
}
function $mkv(n, fields) {
  const fs = $VD[n];
  if (!fs) throw new $TypeErr(`${n} nie ma pól`);
  const out = { $v: n };
  for (const [f, fd] of fs) {
    if (!(f in fields)) throw new $TypeErr(`${n}: brak pola ${f}`);
    out[f] = $conform(fd, fields[f], `${n}.${f}`);
  }
  for (const k of Object.keys(fields)) if (!fs.some(([f]) => f === k)) throw new $TypeErr(`${n} nie ma pola ${k}`);
  return out;
}
function $fields_of(v) {
  const o = {};
  for (const k of Object.keys(v)) if (k[0] !== "$") o[k] = v[k];
  return o;
}
function $with(v, upd) {
  if (v && v.$r) return $mk(v.$r, { ...$fields_of(v), ...upd });
  if (v && v.$v) return $mkv(v.$v, { ...$fields_of(v), ...upd });
  throw new $TypeErr(`with działa na rekordzie, a dostał ${$show(v)}`);
}

function $f(o, name) {
  if (o instanceof $Date) {
    const m = { year: o.y, month: o.m, day: o.d }[name];
    if (m !== undefined) return m;
  } else if (o instanceof $DateTime) {
    const m = { year: o.y, month: o.mo, day: o.d, hour: o.h, minute: o.mi, second: o.s }[name];
    if (m !== undefined) return m;
  } else if (o && typeof o === "object" && name[0] !== "$" && Object.prototype.hasOwnProperty.call(o, name)) {
    return o[name];
  }
  throw new $TypeErr(`${$show(o)} nie ma pola ${name}`);
}

function $isv(v, n) { return !!v && v.$v === n; }
function $try(v, rest) {
  if ($is(rest, v)) throw new $Ret(v);
  return v;
}
function $nomatch(vals, file, line) {
  throw new $TypeErr(`${file}:${line}: match bez pasującej gałęzi dla ${vals.map($show).join(", ")}`);
}
function $orblock_end() {
  throw new $TypeErr("blok po `or` musi zakończyć się return");
}

// ---------- JSON i formularze ----------

class $JNum {
  constructor(t) { this.t = t; }
}

function $json(x) {
  if (typeof x === "string") return JSON.stringify(x);
  if (typeof x === "number" || typeof x === "boolean") return String(x);
  if (x instanceof Dec) return $dec_text(x, 2);
  if (x instanceof $Date) return JSON.stringify($date_text(x));
  if (x instanceof $DateTime) return JSON.stringify($dt_text(x, "T"));
  if (x instanceof $Html) return JSON.stringify(x.s);
  if (Array.isArray(x)) return `[${x.map($json).join(",")}]`;
  if (x && x.$r) return `{${Object.keys($fields_of(x)).map((k) => `${JSON.stringify(k)}:${$json(x[k])}`).join(",")}}`;
  if (x && x.$v) {
    const fs = Object.keys($fields_of(x));
    if (fs.length === 0) return JSON.stringify(x.$v);
    return `{"$v":${JSON.stringify(x.$v)},${fs.map((k) => `${JSON.stringify(k)}:${$json(x[k])}`).join(",")}}`;
  }
  throw new $TypeErr(`nie da się zapisać w JSON: ${$show(x)}`);
}

// JSON z liczbami jako tekst, żeby kwoty nie traciły groszy.
function $jparse(s) {
  let i = 0;
  const bad = () => { throw new $TypeErr(`niepoprawny JSON na pozycji ${i}`); };
  const ws = () => { while (i < s.length && " \t\r\n".includes(s[i])) i++; };
  const val = () => {
    ws();
    const c = s[i];
    if (c === "{") {
      i++; const o = {}; ws();
      if (s[i] === "}") { i++; return o; }
      for (;;) {
        ws(); if (s[i] !== '"') bad();
        const k = str(); ws(); if (s[i] !== ":") bad(); i++;
        o[k] = val(); ws();
        if (s[i] === ",") { i++; continue; }
        if (s[i] === "}") { i++; return o; }
        bad();
      }
    }
    if (c === "[") {
      i++; const a = []; ws();
      if (s[i] === "]") { i++; return a; }
      for (;;) {
        a.push(val()); ws();
        if (s[i] === ",") { i++; continue; }
        if (s[i] === "]") { i++; return a; }
        bad();
      }
    }
    if (c === '"') return str();
    const m = /^-?\d+(\.\d+)?([eE][+-]?\d+)?/.exec(s.slice(i));
    if (m) { i += m[0].length; return new $JNum(m[0]); }
    for (const [w, v] of [["true", true], ["false", false], ["null", null]]) {
      if (s.startsWith(w, i)) { i += w.length; return v; }
    }
    bad();
  };
  const str = () => {
    let j = i + 1;
    while (j < s.length && s[j] !== '"') j += s[j] === "\\" ? 2 : 1;
    if (j >= s.length) bad();
    const r = JSON.parse(s.slice(i, j + 1));
    i = j + 1;
    return r;
  };
  const v = val(); ws();
  if (i !== s.length) bad();
  return v;
}

// buyer_name=Firma&lines[0].name=Usługa → {buyer_name: "Firma", lines: [{name: "Usługa"}]}
function $form_parse(s) {
  const root = {};
  const dec = (x) => {
    try { return decodeURIComponent(x.replace(/\+/g, " ")); } catch { throw new $TypeErr("niepoprawne kodowanie formularza"); }
  };
  for (const pair of s.split("&")) {
    if (!pair) continue;
    const eq = pair.indexOf("=");
    const key = dec(eq < 0 ? pair : pair.slice(0, eq));
    const value = eq < 0 ? "" : dec(pair.slice(eq + 1));
    const path = [];
    for (const m of key.matchAll(/([^.[\]]+)|\[(\d+)\]/g)) path.push(m[2] !== undefined ? Number(m[2]) : m[1]);
    if (path.length === 0) continue;
    let o = root;
    for (let k = 0; k < path.length - 1; k++) {
      const p = path[k];
      if (o[p] === undefined || typeof o[p] !== "object") o[p] = typeof path[k + 1] === "number" ? {} : {};
      o = o[p];
    }
    o[path[path.length - 1]] = value;
  }
  return root;
}

// Obiekt z kluczami 0, 1, ... z formularza albo tablica z JSON.
function $as_array(j) {
  if (Array.isArray(j)) return j;
  if (j && typeof j === "object" && !(j instanceof $JNum)) {
    const ks = Object.keys(j);
    if (ks.every((k) => /^\d+$/.test(k))) return ks.map(Number).sort((a, b) => a - b).map((k) => j[k]);
  }
  return null;
}

function $from_json(d, j, mode) {
  const bad = () => { throw new $TypeErr(`dane nie pasują do ${$tname(d)}: ${j instanceof $JNum ? j.t : JSON.stringify(j)}`); };
  switch (d.k) {
    case "any": return j;
    case "prim": {
      const text = j instanceof $JNum ? j.t : typeof j === "string" && mode === "form" ? j : null;
      switch (d.n) {
        case "Int":
          if (text !== null && /^-?\d+$/.test(text)) return $int(Number(text));
          bad();
        case "Money":
          if (j instanceof $JNum || typeof j === "string") return $dec(j instanceof $JNum ? j.t : j);
          bad();
        case "String": if (typeof j === "string") return j; bad();
        case "Bool": if (typeof j === "boolean") return j; bad();
        case "Date": if (typeof j === "string") return $parse_date(j); bad();
        case "DateTime": if (typeof j === "string") return $parse_dt(j); bad();
        case "Html": bad();
      }
      bad();
    }
    case "list": {
      const a = $as_array(j);
      if (!a) bad();
      return a.map((x) => $from_json(d.e, x, mode));
    }
    case "ref": return $from_json($lookup(d.n), j, mode);
    case "named": return $from_json(d.d, j, mode);
    case "refine": return $conform(d, $from_json(d.b, j, mode), "");
    case "rec": {
      if (!j || typeof j !== "object" || Array.isArray(j) || j instanceof $JNum) bad();
      const fs = {};
      for (const [f, fd] of d.f) {
        if (j[f] === undefined) {
          if ($target_list(fd)) { fs[f] = []; continue; }
          throw new $TypeErr(`${d.n}: brak pola ${f}`);
        }
        fs[f] = $from_json(fd, j[f], mode);
      }
      return $mk(d.n, fs);
    }
    case "var": {
      const fs = $VD[d.n];
      if (!fs) { if (j === d.n) return $V[d.n]; bad(); }
      if (!j || j.$v !== d.n) bad();
      const o = {};
      for (const [f, fd] of fs) o[f] = $from_json(fd, j[f], mode);
      return $mkv(d.n, o);
    }
    case "union":
      for (const a of d.a) {
        try { return $from_json(a, j, mode); } catch (e) { if (!(e instanceof $TypeErr)) throw e; }
      }
      bad();
    case "cap": bad();
  }
  bad();
}
function $target_list(d) {
  for (let i = 0; i < 20; i++) {
    if (d.k === "ref") d = $lookup(d.n);
    else if (d.k === "named") d = d.d;
    else if (d.k === "refine") d = d.b;
    else break;
  }
  return d.k === "list";
}

// ---------- funkcje wbudowane ----------

const $b = {
  segments: (p) => p.split("/").filter((s) => s !== ""),
  parse_int: (s) => {
    const t = s.trim();
    if (!/^-?\d+$/.test(t) || !Number.isSafeInteger(Number(t))) return $V.NotANumber;
    return Number(t);
  },
  parse_money: (s) => {
    const t = s.trim().replace(",", ".");
    if (!/^-?\d+(\.\d+)?$/.test(t)) return $V.NotANumber;
    return $dec(t);
  },
  to_string: (x) => $disp(x),
  to_json: (x) => $json(x),
  round: (x, places) => (x instanceof Dec ? $round_dec(x, places) : x),
  sum: (xs) => xs.reduce((a, b) => $add(a, b), 0),
  distinct: (xs) => {
    const out = [];
    for (const x of xs) if (!out.some((y) => $eq(x, y))) out.push(x);
    return out;
  },
  sort_by: async (xs, fn) => {
    const keyed = [];
    for (const x of xs) keyed.push([await fn(x), x]);
    keyed.sort((a, b) => $cmp(a[0], b[0]));
    return keyed.map((k) => k[1]);
  },
  len: (x) => (typeof x === "string" ? [...x].length : Array.isArray(x) ? x.length : $len_err(x)),
  trim: (s) => s.trim(),
  lower: (s) => s.toLowerCase(),
  upper: (s) => s.toUpperCase(),
  remove: (s, part) => s.split(part).join(""),
  drop_prefix: (s, p) => (s.startsWith(p) ? s.slice(p.length) : s),
  pad_left: (s, n, ch) => s.padStart(n, ch),
  starts_with: (s, p) => s.startsWith(p),
  contains: (s, part) => (Array.isArray(s) ? s.some((x) => $eq(x, part)) : s.includes(part)),
  matches: (s, re) => $regex(re).test(s),
  only_digits: (s) => /^[0-9]+$/.test(s),
  nip_checksum_ok: (s) => {
    if (!/^[0-9]{10}$/.test(s)) return false;
    const w = [6, 5, 7, 2, 3, 4, 5, 6, 7];
    const sum = w.reduce((a, x, i) => a + x * Number(s[i]), 0);
    return sum % 11 === Number(s[9]);
  },
  valid_email: (s) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(s),
  at: (xs, i) => {
    if (!Array.isArray(xs)) throw new $TypeErr(`at działa na liście, a dostał ${$show(xs)}`);
    if (!Number.isInteger(i) || i < 0 || i >= xs.length) throw new $TypeErr(`at: indeks ${i} poza listą o długości ${xs.length}`);
    return xs[i];
  },
  join: (xs, sep) => xs.join(sep),
  split: (s, sep) => s.split(sep),
  chars: (s) => [...s],
  char: (code) => {
    if (!Number.isInteger(code) || code < 0 || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) {
      throw new $TypeErr(`char: ${code} nie jest kodem znaku`);
    }
    return String.fromCodePoint(code);
  },
};
function $len_err(x) { throw new $TypeErr(`len działa na tekście i liście, a dostał ${$show(x)}`); }
const $regexes = new Map();
function $regex(re) {
  let r = $regexes.get(re);
  if (!r) { r = new RegExp(re, "u"); $regexes.set(re, r); }
  return r;
}

// ---------- metody: listy i uprawnienia ----------

async function $call(o, name, pos, named, targs, line) {
  const arg = (i, key) => (key && named[key] !== undefined ? named[key] : pos[i]);
  if (Array.isArray(o)) {
    switch (name) {
      case "map": { const out = []; for (const x of o) out.push(await pos[0](x)); return out; }
      case "filter": { const out = []; for (const x of o) if ($bool(await pos[0](x))) out.push(x); return out; }
      case "reverse": return [...o].reverse();
    }
  } else if (o && o.$cap) {
    const m = o[name];
    if (typeof m === "function" && !(o.$cap === "DbRead" && name !== "get" && name !== "all")) {
      return await m.call(o, arg, targs);
    }
  }
  throw new $TypeErr(`linia ${line}: ${$show(o)} nie ma metody ${name}`);
}

function $is_sqlite_err(e) {
  return !!e && (e.name === "SQLiteError" || e.constructor?.name === "SQLiteError" || String(e.code || "").startsWith("SQLITE"));
}

function $mkdb(path) {
  const sql = new Database(path, { create: true });
  if (path !== ":memory:") sql.exec("PRAGMA journal_mode = WAL");
  sql.exec("CREATE TABLE IF NOT EXISTS sowa_kv (collection TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY (collection, key))");
  const st = {
    get: sql.query("SELECT value FROM sowa_kv WHERE collection = ?1 AND key = ?2"),
    all: sql.query("SELECT value FROM sowa_kv WHERE collection = ?1 ORDER BY key"),
    save: sql.query("INSERT INTO sowa_kv (collection, key, value) VALUES (?1, ?2, ?3) ON CONFLICT (collection, key) DO UPDATE SET value = excluded.value"),
  };
  const shared = { sql, st, lock: Promise.resolve(), sp: 0 };
  return $db_handle(shared, 0);
}

// Transakcje na jednym połączeniu idą po kolei; zagnieżdżona transakcja to SAVEPOINT.
function $db_handle(sh, depth) {
  return {
    $cap: "Db",
    get(arg, targs) {
      const row = sh.st.get.get(arg(0, "collection"), $disp(arg(1, "key")));
      if (!row) return $V.NoRow;
      try { return $from_json(targs[0] || $P.Any, $jparse(row.value), "json"); } catch (e) {
        if (e instanceof $TypeErr) return $V.DbError;
        throw e;
      }
    },
    all(arg, targs) {
      return sh.st.all.all(arg(0, "collection")).map((r) => $from_json(targs[0] || $P.Any, $jparse(r.value), "json"));
    },
    save(arg) {
      sh.st.save.run(arg(0, "collection"), $disp(arg(1, "key")), $json(arg(2, "value")));
    },
    async transaction(arg) {
      const fn = arg(0);
      const run = async () => {
        const name = `sp${++sh.sp}`;
        sh.sql.exec(`SAVEPOINT ${name}`);
        try {
          const r = await fn($db_handle(sh, depth + 1));
          sh.sql.exec(`RELEASE ${name}`);
          return r;
        } catch (e) {
          sh.sql.exec(`ROLLBACK TO ${name}`);
          sh.sql.exec(`RELEASE ${name}`);
          if ($is_sqlite_err(e)) return $V.DbError;
          throw e;
        }
      };
      if (depth > 0) return run();
      const p = sh.lock.then(run);
      sh.lock = p.catch(() => {});
      return p;
    },
  };
}

function $now_dt() {
  const d = new Date();
  return new $DateTime(d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds());
}
function $mkclock(fixed) {
  const now = fixed ? $parse_dt(fixed) : null;
  return {
    $cap: "Clock",
    now() { return now || $now_dt(); },
    today() { const t = now || $now_dt(); return new $Date(t.y, t.mo, t.d); },
  };
}

function $mkhttp(spec) {
  const send = async (method, path, body) => {
    if (spec.fake) {
      const fake = $FNS[spec.fake];
      if (!fake) throw new $TypeErr(`nie ma atrapy ${spec.fake}`);
      return await fake($mk("HttpRequest", { method: $V[method], path, body }));
    }
    const headers = { "content-type": "application/json" };
    if (spec.token_env && process.env[spec.token_env]) headers.authorization = `Bearer ${process.env[spec.token_env]}`;
    try {
      const r = await fetch(spec.url.replace(/\/$/, "") + path, {
        method: method.toUpperCase(),
        headers,
        body: method === "Get" ? undefined : body,
        signal: AbortSignal.timeout(10000),
      });
      return $mk("HttpResponse", { status: r.status, body: await r.text() });
    } catch (e) {
      console.error(`[sowa] ${spec.url}${path}: ${e.message}`);
      return $V.HttpError;
    }
  };
  return {
    $cap: "Http",
    post(arg) { return send("Post", arg(0, "path"), arg(1, "body")); },
    get(arg) { return send("Get", arg(0, "path"), ""); },
  };
}

// Bun nie ma klienta SMTP, więc w tym backendzie działa tylko server = "memory".
function $mkmailer(spec) {
  if (!spec.server) throw new $TypeErr("Mailer: brak server w sowa.toml");
  return {
    $cap: "Mailer",
    send(arg) {
      if (spec.server === "memory") return $V.Sent;
      console.error(`[sowa] smtp ${spec.server}: wysyłka działa tylko w backendzie Rust`);
      return $V.MailTimeout;
    },
  };
}

const $STATUS = { Ok: 200, Redirect: 303, BadRequest: 400, NotFound: 404, BadGateway: 502 };
const $METHODS = { GET: "Get", HEAD: "Get", POST: "Post", PUT: "Put", PATCH: "Patch", DELETE: "Delete" };

function $mkserver(spec) {
  const listen = spec.listen || "127.0.0.1:8080";
  const i = listen.lastIndexOf(":");
  const hostname = listen.slice(0, i), port = Number(listen.slice(i + 1));
  return {
    $cap: "Server",
    async serve(arg) {
      const handler = arg(0);
      const reqType = $T.Request && $T.Request.k === "rec" ? "Request" : "HttpRequest";
      const server = Bun.serve({
        hostname,
        port,
        async fetch(r) {
          const url = new URL(r.url);
          const t0 = performance.now();
          let status = 500;
          try {
            const method = $METHODS[r.method];
            if (!method) return new Response("", { status: (status = 405) });
            const req = $mk(reqType, { method: $V[method], path: url.pathname, body: method === "Get" ? "" : await r.text() });
            const res = await handler(req);
            const name = res && res.$v;
            status = $STATUS[name] || 500;
            const headers = { "content-type": "text/html; charset=utf-8" };
            if (name === "Redirect") return new Response("", { status, headers: { location: res.to } });
            const body = res && res.body !== undefined ? (res.body instanceof $Html ? res.body.s : $disp(res.body)) : "";
            if (!$STATUS[name]) console.error(`[sowa] nieznana odpowiedź: ${$show(res)}`);
            return new Response(r.method === "HEAD" ? null : body, { status, headers });
          } catch (e) {
            console.error(`[sowa] błąd programu: ${e instanceof $TypeErr ? e.message : e.stack}`);
            return new Response("Błąd programu.", { status: 500, headers: { "content-type": "text/plain; charset=utf-8" } });
          } finally {
            console.log(`${r.method} ${url.pathname} → ${status} (${(performance.now() - t0).toFixed(1)} ms)`);
          }
        },
      });
      console.log(`Sowa: serwer na http://${hostname === "0.0.0.0" ? "localhost" : hostname}:${server.port}`);
      return new Promise(() => {});
    },
  };
}

// Random: xoshiro256++ z ziarnem rozwiniętym przez SplitMix64, ten sam co w ttfx,
// więc przy tym samym seed obie implementacje losują te same liczby.
const $U64 = (1n << 64n) - 1n;
function $rotl64(x, k) { return ((x << k) | (x >> (64n - k))) & $U64; }
function $mkrandom(seed) {
  let sm = seed === undefined ? BigInt.asUintN(64, BigInt(Date.now()) * 1000003n) : BigInt.asUintN(64, BigInt(seed));
  const split = () => {
    sm = (sm + 0x9e3779b97f4a7c15n) & $U64;
    let z = sm;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & $U64;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & $U64;
    return z ^ (z >> 31n);
  };
  let s0 = split(), s1 = split(), s2 = split(), s3 = split();
  const next = () => {
    const r = ($rotl64((s0 + s3) & $U64, 23n) + s0) & $U64;
    const t = (s1 << 17n) & $U64;
    s2 ^= s0; s3 ^= s1; s1 ^= s2; s0 ^= s3; s2 ^= t;
    s3 = $rotl64(s3, 45n);
    return r;
  };
  const below = (n) => {
    const bits = Math.max(n === 1 ? 0 : (n - 1).toString(2).length, 1);
    const shift = BigInt(64 - bits);
    for (;;) { const r = Number(next() >> shift); if (r < n) return r; }
  };
  return {
    $cap: "Random",
    int(arg) {
      const lo = arg(0, "min"), hi = arg(1, "max");
      if (!Number.isInteger(lo) || !Number.isInteger(hi) || lo > hi) throw new $TypeErr(`random.int: pusty przedział ${lo}..${hi}`);
      return lo + below(hi - lo + 1);
    },
    choice(arg) {
      const xs = arg(0, "list");
      if (!Array.isArray(xs) || xs.length === 0) throw new $TypeErr(`random.choice: pusta lista`);
      return xs[below(xs.length)];
    },
  };
}

// Terminal: całe stdin naraz i stdout z buforem; $run_main opróżnia bufor na końcu.
const $flushes = [];
function $mkterminal() {
  const fs = require("node:fs");
  let buf = [], size = 0;
  const flush = () => { if (size) { fs.writeSync(1, buf.join("")); buf = []; size = 0; } };
  $flushes.push(flush);
  return {
    $cap: "Terminal",
    async read() { return await Bun.stdin.text(); },
    write(arg) {
      const t = arg(0, "text");
      buf.push(t); size += t.length;
      if (size > 1 << 16) flush();
    },
    exit(arg) { flush(); process.exit(arg(0, "code")); },
  };
}

function $mkres(spec, root) {
  switch (spec.type) {
    case "Db": {
      const url = spec.url || "memory";
      if (url === "memory") return $mkdb(":memory:");
      if (url.startsWith("sqlite:")) {
        const p = url.slice("sqlite:".length);
        return $mkdb(p.startsWith("/") ? p : root + "/" + p);
      }
      throw new $TypeErr(`Db: nieobsługiwany adres ${url} (tylko sqlite:plik i memory)`);
    }
    case "Clock": return $mkclock(spec.now);
    case "Http": return $mkhttp(spec);
    case "Mailer": return $mkmailer(spec);
    case "Server": return $mkserver(spec);
    case "Random": return $mkrandom(spec.seed ?? (spec.seed_env ? process.env[spec.seed_env] : undefined));
    case "Terminal": return $mkterminal();
  }
  throw new $TypeErr(`nieznany zasób ${spec.type}`);
}

async function $run_main(main, names, specs, root) {
  const args = names.map((n) => $mkres(specs[n], root));
  try {
    await main(...args);
    for (const f of $flushes) f();
  } catch (e) {
    for (const f of $flushes) f();
    console.error(`błąd programu: ${e instanceof $TypeErr ? e.message : e.stack}`);
    process.exit(1);
  }
}

// ---------- testy ----------

function $check(v, src, line) {
  if (v !== true) throw new $Fail(`${src}\n    wynik: ${$show(v)}`);
}
function $check_eq(a, b, src, line) {
  if (!$eq(a, b)) throw new $Fail(`${src}\n    lewa strona:  ${$show(a)}\n    prawa strona: ${$show(b)}`);
}
function $check_is(a, d, neg, src, line) {
  if ($is(d, a) === neg) throw new $Fail(`${src}\n    wartość: ${$show(a)}`);
}

function $rng(seed) {
  let s = seed >>> 0;
  const next = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    f: next,
    int: (lo, hi) => lo + Math.floor(next() * (hi - lo + 1)),
    pick: (a) => a[Math.floor(next() * a.length)],
  };
}
function $hash(s) {
  let h = 2166136261;
  for (const c of s) h = Math.imul(h ^ c.codePointAt(0), 16777619);
  return h >>> 0;
}

const $STR_POOL = ["", "a", "Usługa", "Zażółć gęślą jaźń", "<script>alert(1)</script>", "a&b", '"cudzysłów"', "'", " spacja ", "FV/2026/0001", "1234563218", "x".repeat(201)];
const $CHARS = "abcXYZ019 ąęłśżź<>&\"'/-.@";

function $rand_str(g, lo = 0, hi = 12) {
  const n = g.int(lo, hi);
  let s = "";
  for (let i = 0; i < n; i++) s += g.pick([...$CHARS]);
  return s;
}

// Mały generator tekstu z wyrażenia regularnego: znaki, klasy [...], \d, kwantyfikatory {n}, {n,}, {n,m}, ?, *, +.
function $gen_re(re, g) {
  let i = 0, out = "";
  if (re[i] === "^") i++;
  while (i < re.length) {
    if (re[i] === "$" && i === re.length - 1) break;
    let chars;
    if (re[i] === "[") {
      const j = re.indexOf("]", i);
      const body = re.slice(i + 1, j);
      chars = [];
      for (let k = 0; k < body.length; k++) {
        if (body[k + 1] === "-" && k + 2 < body.length) {
          for (let c = body.charCodeAt(k); c <= body.charCodeAt(k + 2); c++) chars.push(String.fromCharCode(c));
          k += 2;
        } else chars.push(body[k]);
      }
      i = j + 1;
    } else if (re[i] === "\\") {
      const c = re[i + 1];
      chars = c === "d" ? [..."0123456789"] : [c];
      i += 2;
    } else {
      chars = [re[i]];
      i++;
    }
    let lo = 1, hi = 1;
    const q = /^\{(\d+)(,(\d*))?\}/.exec(re.slice(i));
    if (q) {
      lo = +q[1];
      hi = q[2] ? (q[3] ? +q[3] : lo + 3) : lo;
      i += q[0].length;
    } else if (re[i] === "?") { lo = 0; hi = 1; i++; }
    else if (re[i] === "*") { lo = 0; hi = 4; i++; }
    else if (re[i] === "+") { lo = 1; hi = 4; i++; }
    const n = g.int(lo, hi);
    for (let k = 0; k < n; k++) out += g.pick(chars);
  }
  return out;
}

function $gen_base(d, g, h, depth) {
  const r = $resolve(d);
  if (r.k === "prim") {
    switch (r.n) {
      case "Int": {
        if (h.min !== undefined || h.max !== undefined) {
          const lo = h.min !== undefined ? Math.ceil(Number(h.min)) + (h.min_ex && Number.isInteger(Number(h.min)) ? 1 : 0) : Number(h.max) - 1000;
          const hi = h.max !== undefined ? Math.floor(Number(h.max)) - (h.max_ex && Number.isInteger(Number(h.max)) ? 1 : 0) : lo + 1000;
          return g.f() < 0.2 ? g.pick([lo, hi]) : g.int(lo, hi);
        }
        break;
      }
      case "Money": {
        if (h.min !== undefined || h.max !== undefined) {
          const lo = h.min !== undefined ? Number(h.min) : Number(h.max) - 1000;
          const hi = h.max !== undefined ? Number(h.max) : lo + 1000;
          const cents = g.int(Math.ceil(lo * 100), Math.floor(hi * 100));
          return $div(cents, $dec("100"));
        }
        break;
      }
      case "String": {
        if (h.re) return $gen_re(h.re, g);
        if (h.digits) {
          const n = h.len !== undefined ? h.len : g.int(1, 12);
          let s = "";
          for (let i = 0; i < n; i++) s += String(g.int(0, 9));
          return s;
        }
        if (h.email) return `${$rand_str(g, 1, 6).replace(/[\s@<>&"']/g, "") || "a"}@firma.pl`;
        if (h.prefix !== undefined) return h.prefix + $rand_str(g);
        if (h.len !== undefined || h.minlen !== undefined || h.maxlen !== undefined) {
          const lo = h.len ?? h.minlen ?? 0, hi = h.len ?? Math.min(h.maxlen ?? lo + 12, lo + 30);
          return g.f() < 0.3 ? g.pick($STR_POOL) : $rand_str(g, lo, hi);
        }
        break;
      }
    }
  }
  if (r.k === "list" && (h.len !== undefined || h.minlen !== undefined || h.maxlen !== undefined)) {
    const lo = h.len ?? h.minlen ?? 0, hi = h.len ?? Math.min(h.maxlen ?? lo + 3, lo + 3);
    const n = g.int(lo, hi);
    const out = [];
    for (let i = 0; i < n; i++) out.push($gen(r.e, g, depth + 1));
    return out;
  }
  return $gen(d, g, depth);
}

function $resolve(d) {
  for (let i = 0; i < 20; i++) {
    if (d.k === "ref") d = $lookup(d.n);
    else if (d.k === "named") d = d.d;
    else break;
  }
  return d;
}

function $gen(d, g, depth = 0) {
  switch (d.k) {
    case "prim":
      switch (d.n) {
        case "Int": return g.f() < 0.3 ? g.pick([0, 1, -1, 2, 7, 42, 100, 2026]) : g.int(-1000, 10000);
        case "Money": return g.f() < 0.3 ? $dec(g.pick(["0", "0.01", "1", "33.33", "100", "999999.99"])) : $div(g.int(-10000, 1000000), $dec("100"));
        case "String": return g.f() < 0.5 ? g.pick($STR_POOL) : $rand_str(g);
        case "Bool": return g.f() < 0.5;
        case "Html": return new $Html($esc($rand_str(g)));
        case "Date": { const y = g.int(2000, 2030), m = g.int(1, 12); return new $Date(y, m, g.int(1, 28)); }
        case "DateTime": { const y = g.int(2000, 2030), m = g.int(1, 12); return new $DateTime(y, m, g.int(1, 28), g.int(0, 23), g.int(0, 59), g.int(0, 59)); }
      }
      break;
    case "list": {
      const n = depth > 3 ? g.int(0, 1) : g.int(0, 4);
      const out = [];
      for (let i = 0; i < n; i++) out.push($gen(d.e, g, depth + 1));
      return out;
    }
    case "ref": return $gen($lookup(d.n), g, depth);
    case "named": return $gen(d.d, g, depth);
    case "rec": {
      const fs = {};
      for (const [f, fd] of d.f) fs[f] = $gen(fd, g, depth + 1);
      return $mk(d.n, fs);
    }
    case "var": {
      const fs = $VD[d.n];
      if (!fs) return $V[d.n];
      const o = {};
      for (const [f, fd] of fs) o[f] = $gen(fd, g, depth + 1);
      return $mkv(d.n, o);
    }
    case "union": return $gen(g.pick(d.a), g, depth);
    case "refine": {
      for (let i = 0; i < 2000; i++) {
        try {
          const x = $conform(d.b, $gen_base(d.b, g, d.h || {}, depth), "");
          if (d.c(x) === true) return x;
        } catch (e) {
          if (!(e instanceof $TypeErr)) throw e;
        }
      }
      throw new $TypeErr(`property: nie udało się wylosować wartości typu ${d.src}`);
    }
  }
  throw new $TypeErr(`property: nie da się wylosować wartości typu ${$tname(d)}`);
}

function $test_res(spec) {
  const R = {};
  for (const n of Object.keys(spec)) R[n] = $mkres(spec[n], ".");
  return R;
}

function $explain(e) {
  if (e instanceof $Fail) return e.message;
  if (e instanceof $Ret) return `try: wynik to błąd ${$show(e.v)}`;
  if (e instanceof $TypeErr) return `błąd programu: ${e.message}`;
  return `błąd: ${e && e.stack ? e.stack : e}`;
}

async function $run_tests(tests, spec, quiet) {
  const passed = { example: 0, property: 0, doc: 0 };
  const failed = [];
  for (const t of tests) {
    const where = `${t.file}:${t.line}`;
    if (t.kind === "property") {
      const g = $rng($hash(where));
      let ok = true;
      for (let i = 0; i < 100 && ok; i++) {
        let vals;
        try {
          vals = {};
          for (const [n, d] of t.gens()) vals[n] = $gen(d, g);
          await t.run($test_res(spec), vals);
        } catch (e) {
          ok = false;
          const shown = vals ? Object.entries(vals).map(([k, x]) => `\n    ${k} = ${$show(x)}`).join("") : "";
          failed.push(`${where}: property ${t.src}\n    przypadek ${i + 1} ze 100:${shown}\n    ${$explain(e).replace(/\n/g, "\n    ")}`);
        }
      }
      if (ok) passed.property++;
    } else {
      try {
        await t.run($test_res(spec));
        passed[t.kind]++;
      } catch (e) {
        failed.push(`${where}: ${t.kind === "doc" ? "blok sowa" : "przykład"}${t.fn ? " (" + t.fn + ")" : ""}\n    ${$explain(e)}`);
      }
    }
  }
  for (const f of failed) console.log(`BŁĄD ${f}\n`);
  const total = tests.length;
  const summary = `przykłady: ${passed.example}, property: ${passed.property} (po 100 przypadków), bloki sowa w docs/: ${passed.doc}`;
  if (failed.length) {
    console.log(`${failed.length} z ${total} testów nie przechodzi. Przechodzą: ${summary}`);
    process.exit(1);
  }
  console.log(`Wszystkie testy przechodzą (${total}): ${summary}`);
}
