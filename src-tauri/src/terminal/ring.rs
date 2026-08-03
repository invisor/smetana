//! Кольцо сырых байтов: то, что видел человек, и то, чем перерисовывается
//! xterm.js при подключении к сессии. Разбор экрана к нему отношения не имеет
//! — это screen.rs, и он читает тот же поток независимо.

use std::collections::VecDeque;

pub struct Ring {
    buf: VecDeque<u8>,
    cap: usize,
    /// Что-то уже выпало из начала — значит снимок начинается с середины
    /// сессии, и открытые в выпавшем куске атрибуты надо погасить.
    dropped: bool,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self { buf: VecDeque::new(), cap, dropped: false }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend(bytes);
        if self.buf.len() <= self.cap {
            return;
        }
        self.dropped = true;
        let excess = self.buf.len() - self.cap;
        self.buf.drain(..excess);
        // Дорезаем до ближайшего перевода строки. Без этого снимок начинался
        // бы с середины escape-последовательности, и первая строка после
        // долгой сессии приезжала бы мусором — выглядит как сломанный
        // терминал, а сломан один байт.
        if let Some(nl) = self.buf.iter().position(|b| *b == b'\n') {
            self.buf.drain(..=nl);
        } else {
            self.buf.clear();
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.buf.len() + 4);
        if self.dropped {
            out.extend_from_slice(b"\x1b[0m");
        }
        out.extend(self.buf.iter().copied());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn пока_влезает_отдаёт_всё_как_есть() {
        let mut ring = Ring::new(64);
        ring.push(b"hello\nworld\n");
        assert_eq!(ring.snapshot(), b"hello\nworld\n");
    }

    #[test]
    fn переполнение_режет_по_переводу_строки() {
        let mut ring = Ring::new(16);
        // 20 байт при потолке 16: лишние четыре уходят, и обрезка доводит
        // начало до перевода строки — целая строка теряется целиком, а не
        // наполовину.
        ring.push(b"aaaa\nbbbb\ncccc\ndddd\n");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        // Сброс атрибутов впереди — открытая последовательность цвета
        // осталась в выпавшей части, и без сброса она красила бы всё дальше.
        assert!(text.starts_with("\u{1b}[0m"), "нет сброса атрибутов: {text:?}");
        assert_eq!(text.trim_start_matches("\u{1b}[0m"), "bbbb\ncccc\ndddd\n");
    }

    #[test]
    fn обрезка_не_рвёт_escape_последовательность() {
        let mut ring = Ring::new(12);
        ring.push(b"\x1b[31mred text here\nplain\n");
        let text = String::from_utf8(ring.snapshot()).unwrap();
        let body = text.trim_start_matches("\u{1b}[0m");
        assert!(!body.contains("[31"), "хвост escape-последовательности уехал в снимок: {body:?}");
    }
}
