//! Texto de fondo para el estilo de código: salida falsa de compilaciones, pruebas, logs,
//! `git log`, volcados hexadecimales y código. Avanza una línea cada pocos segundos fuera
//! del juego, como una terminal que de verdad está haciendo algo.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const KEEP: usize = 240;

const CRATES: [&str; 20] = [
    "serde", "tokio", "hyper", "rand", "regex", "syn", "quote", "proc-macro2", "libc", "anyhow", "thiserror",
    "tracing", "bytes", "futures", "mio", "clap", "itoa", "memchr", "log", "smallvec",
];
const MODS: [&str; 10] =
    ["parser", "cache", "net", "store", "sched", "codec", "auth", "index", "worker", "config"];
const FNS: [&str; 12] = [
    "parse_header", "flush_pending", "resolve_deps", "encode_frame", "spawn_worker", "load_index", "merge_ranges",
    "poll_ready", "drain_queue", "validate_token", "build_graph", "apply_patch",
];
const VARS: [&str; 10] = ["buf", "ctx", "len", "idx", "state", "node", "cfg", "chunk", "offset", "handle"];
const TYPES: [&str; 8] = ["Frame", "Session", "Graph", "Token", "Header", "Queue", "Index", "Config"];
const MSGS: [&str; 10] = [
    "fix off-by-one in range merge",
    "refactor worker pool shutdown",
    "add retry with backoff",
    "bump deps",
    "handle empty chunk",
    "cache resolved paths",
    "tighten token validation",
    "split codec into modules",
    "log slow queries",
    "update lockfile",
];
const INFO: [&str; 8] = [
    "request completed",
    "flushed segment",
    "cache warm",
    "rebuilt index",
    "connection reused",
    "checkpoint written",
    "worker idle",
    "compacted log",
];

pub struct Ambient {
    lines: VecDeque<String>,
    rng: fastrand::Rng,
    /// Reloj falso para las marcas de hora de los logs (segundos del día).
    clock: u32,
    next: Option<Instant>,
}

impl Ambient {
    pub fn new(rng: fastrand::Rng) -> Ambient {
        let mut a = Ambient { lines: VecDeque::with_capacity(KEEP), rng, clock: 0, next: None };
        a.clock = a.rng.u32(8 * 3600..18 * 3600);
        for _ in 0..KEEP {
            let l = a.make_line();
            a.lines.push_back(l);
        }
        a
    }

    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.rng.usize(..v.len())]
    }

    fn version(&mut self) -> String {
        format!("{}.{}.{}", self.rng.u8(0..2), self.rng.u8(0..40), self.rng.u8(0..20))
    }

    fn hex(&mut self, n: usize) -> String {
        (0..n).map(|_| char::from_digit(self.rng.u32(0..16), 16).unwrap_or('0')).collect()
    }

    fn stamp(&mut self) -> String {
        self.clock = (self.clock + self.rng.u32(0..4)) % 86_400;
        format!("{:02}:{:02}:{:02}", self.clock / 3600, self.clock % 3600 / 60, self.clock % 60)
    }

    /// Una línea de salida falsa.
    pub fn make_line(&mut self) -> String {
        match self.rng.u8(0..20) {
            0..=3 => {
                let c = self.pick(&CRATES);
                format!("   Compiling {c} v{}", self.version())
            }
            4..=5 => {
                let c = self.pick(&CRATES);
                format!("    Checking {c} v{}", self.version())
            }
            6..=7 => {
                let (m, f) = (self.pick(&MODS), self.pick(&FNS));
                format!("test {m}::tests::{f} ... ok")
            }
            8..=10 => {
                let (t, m, msg) = (self.stamp(), self.pick(&MODS), self.pick(&INFO));
                format!("[{t}] INFO  {m}: {msg} in {}ms", self.rng.u32(1..900))
            }
            11 => {
                let (t, m, k) = (self.stamp(), self.pick(&MODS), self.hex(8));
                format!("[{t}] DEBUG {m}: cache hit key={k}")
            }
            12..=13 => {
                let (h, msg) = (self.hex(7), self.pick(&MSGS));
                format!("{h} {msg}")
            }
            14 => {
                let addr = self.hex(8);
                let bytes: Vec<String> = (0..8).map(|_| self.hex(2)).collect();
                format!("0x{addr}  {}", bytes.join(" "))
            }
            15 => {
                let (f, t) = (self.pick(&FNS), self.pick(&TYPES));
                format!("fn {f}(&mut self) -> Result<{t}, Error> {{")
            }
            16 => {
                let (v, f, v2) = (self.pick(&VARS), self.pick(&FNS), self.pick(&VARS));
                format!("    let {v} = self.{f}(&{v2})?;")
            }
            17 => {
                let (t, t2) = (self.pick(&TYPES), self.pick(&TYPES));
                format!("impl From<{t}> for {t2} {{")
            }
            18 => {
                let v = self.pick(&VARS);
                format!("warning: unused variable: `{v}`")
            }
            _ => format!(
                "    Finished `dev` profile [unoptimized + debuginfo] target(s) in {}.{:02}s",
                self.rng.u8(0..40),
                self.rng.u8(0..100)
            ),
        }
    }

    /// Las últimas `n` líneas, de la más vieja a la más nueva.
    pub fn tail(&self, n: usize) -> impl Iterator<Item = &str> {
        self.lines.iter().skip(self.lines.len().saturating_sub(n)).map(|s| s.as_str())
    }

    pub fn due(&self) -> Option<Instant> {
        self.next
    }

    /// Programa la próxima línea dentro de 1.2–3.5 s si no hay una programada.
    pub fn schedule(&mut self, now: Instant) {
        if self.next.is_none() {
            self.next = Some(now + Duration::from_millis(self.rng.u64(1200..=3500)));
        }
    }

    pub fn stop(&mut self) {
        self.next = None;
    }

    /// Añade una línea si ya tocaba. Devuelve si cambió algo.
    pub fn advance(&mut self, now: Instant) -> bool {
        match self.next {
            Some(t) if now >= t => {
                let l = self.make_line();
                self.lines.push_back(l);
                if self.lines.len() > KEEP {
                    self.lines.pop_front();
                }
                self.next = None;
                self.schedule(now);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_look_like_output_and_are_bounded() {
        let mut a = Ambient::new(fastrand::Rng::with_seed(1));
        assert_eq!(a.tail(1000).count(), KEEP);
        for _ in 0..500 {
            let l = a.make_line();
            assert!(!l.is_empty() && l.len() < 100, "{l}");
        }
    }

    #[test]
    fn scrolls_only_when_due() {
        let mut a = Ambient::new(fastrand::Rng::with_seed(2));
        let t0 = Instant::now();
        assert!(!a.advance(t0));
        a.schedule(t0);
        let due = a.due().unwrap();
        assert!(due >= t0 + Duration::from_millis(1200));
        let last = a.tail(1).next().unwrap().to_string();
        assert!(!a.advance(t0));
        assert!(a.advance(due));
        assert_ne!(a.tail(1).next().unwrap(), last);
        assert!(a.due().is_some());
        a.stop();
        assert!(a.due().is_none());
    }
}
