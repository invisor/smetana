//! Экран сессии: то, что приложение читает, чтобы понять происходящее.
//! Сырой поток агента — каша из перемещений курсора и перерисовок, в ней
//! нечего искать; экран — это текст, который видит человек.

pub struct Screen {
    parser: vt100::Parser,
}

impl Screen {
    pub fn new(cols: u16, rows: u16) -> Self {
        // Прокрутка назад здесь не нужна: для человека её держит кольцо,
        // а распознаванию хватает видимого экрана.
        Self { parser: vt100::Parser::new(rows, cols, 0) }
    }

    /// Скормить кусок вывода. Возвращает `true`, если в куске был звонок:
    /// парсер его поглощает, а слою A он нужен.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        self.parser.process(bytes);
        bytes.contains(&0x07)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        // set_size живёт на vt100::Screen, а не на Parser.
        self.parser.screen_mut().set_size(rows, cols);
    }

    pub fn lines(&self) -> Vec<String> {
        let screen = self.parser.screen();
        let (_, cols) = screen.size();
        // rows(start_col, width) — вопреки имени, это не индекс строки, а окно
        // по столбцам: итератор уже проходит по всем видимым строкам целиком,
        // отдавая для каждой её текст в столбцах [start_col, start_col+width).
        screen.rows(0, cols).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn показывает_то_что_напечатали() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello\r\nworld\r\n");
        let lines = screen.lines();
        assert_eq!(lines[0].trim_end(), "hello");
        assert_eq!(lines[1].trim_end(), "world");
    }

    #[test]
    fn перерисовка_курсором_видна_как_итог() {
        let mut screen = Screen::new(20, 4);
        // Напечатали, вернулись в начало строки, перепечатали — на экране
        // должно остаться только второе. Именно из-за этого сырой поток
        // регуляркой не читается, а экран читается.
        screen.feed(b"thinking...\r");
        screen.feed(b"done       ");
        assert_eq!(screen.lines()[0].trim_end(), "done");
    }

    #[test]
    fn звонок_замечен_и_не_попал_на_экран() {
        let mut screen = Screen::new(20, 4);
        assert!(screen.feed(b"ping\x07"), "BEL не замечен");
        assert!(!screen.lines()[0].contains('\u{7}'));
        assert!(!screen.feed(b"quiet"), "BEL померещился");
    }

    #[test]
    fn смена_размера_переносит_содержимое() {
        let mut screen = Screen::new(10, 3);
        screen.feed(b"abc");
        screen.resize(40, 10);
        assert_eq!(screen.lines().len(), 10);
        assert_eq!(screen.lines()[0].trim_end(), "abc");
    }
}
