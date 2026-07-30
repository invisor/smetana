# План реализации: живая синхронизация bd с канбан-доской

> **Для агентов-исполнителей:** ОБЯЗАТЕЛЬНАЯ ПОД-СКИЛЛ: используйте
> `superpowers:subagent-driven-development` (рекомендуется) или
> `superpowers:executing-plans`, чтобы выполнять план задача за задачей. Шаги
> размечены чекбоксами (`- [ ]`).

**Цель:** доска в `views/DesktopApp.vue` показывает реальное содержимое трекера bd, обновляется сама
при любом изменении и позволяет заводить, править, закрывать и переоткрывать задачи.

**Архитектура:** прослойка целиком живёт в Rust-процессе Tauri. Один tokio-воркер владеет снимком
трекера, сериализует вызовы CLI `bd` и рассылает во фронт только дельты. Изменения замечаются через
fs-watch каталога `.beads` с инкрементальной догрузкой и периодической полной страховкой. Сам
бинарник `bd` едет в бандле как sidecar, поэтому приложение не зависит от того, установлен ли
трекер на машине.

**Стек:** Tauri 2, Rust (`notify`, `tokio`, `serde`, `thiserror`), Vue 3 + Vite, bd 1.1.2.

**Спецификация:** `docs/superpowers/specs/2026-07-31-bd-tracker-sync-design.md`

**Ветка:** всю работу вести в `feat/bd-tracker-sync`, отведённой от `main`.

## Глобальные ограничения

Эти правила действуют в каждой задаче, повторять их в шагах не нужно.

- **Никаких CSS-классов и блоков `<style>`.** Каждое визуальное значение — вычисляемый объект стилей
  в `:style`, каждое значение внутри — ссылка `var(--token)`. Нет подходящего токена — это вопрос к
  дизайн-системе, а не повод написать `#hex` или `8px`.
- **Тест-раннера для фронта в репозитории нет, и заводить его нельзя.** Фронт проверяется руками:
  `npm run dev`, все четыре сочетания `?theme=dark|light` × `?density=comfortable|compact`, плюс
  `?view=gallery`. Для Rust используется штатный `cargo test`.
- **Никаких градиентов, изображений, стекла, размытия и эмодзи.**
- Регистр предложений везде; идентификаторы моноширинным (`--font-mono`), проза — гротеском.
- Цель сборки фронта — `es2021`, `chrome100`, `safari15`. Не поднимать.
- Новый компонент экспортируется из `src/components/index.js` и добавляется в `views/Gallery.vue`.
  Продуктовый код импортирует из `index.js`, компоненты друг друга — относительными путями.
- Новая иконка сначала регистрируется в `src/components/core/icons.js`, иначе `Icon` ругается в dev.
- Громкий уровень (`loud`) бюджетируется в 1–2 элемента на экран.
- Версия bd задаётся ровно в одном месте — константа `BD_VERSION` в `scripts/fetch-bd.mjs`,
  сейчас `1.1.2`.
- Все команды в шагах — неинтерактивные. Никаких `-i` у файловых операций, `--ci`/`-y` у
  генераторов; интерактивный промпт подвешивает исполнителя.

## Структура файлов

| файл | ответственность |
|---|---|
| `scripts/fetch-bd.mjs` | скачивает релиз bd, сверяет sha256, кладёт под именем с target triple |
| `src-tauri/tauri.conf.json` | конфигурация приложения, `externalBin` |
| `src-tauri/src/lib.rs` | сборка приложения: плагин shell, состояние, регистрация команд |
| `src-tauri/src/tracker/model.rs` | `Issue`, `Dependency`, `ColumnDef`, `Delta`, `Snapshot`, `TrackerError` |
| `src-tauri/src/tracker/bd.rs` | единственное место, знающее CLI: сборка аргументов, запуск, разбор |
| `src-tauri/src/tracker/store.rs` | снимок трекера и вычисление дельты |
| `src-tauri/src/tracker/watcher.rs` | fs-watch по `.beads`, фильтр значимых путей |
| `src-tauri/src/tracker/service.rs` | воркер: очередь запросов, debounce тиков, рассылка событий |
| `src-tauri/src/tracker/commands.rs` | тонкие `#[tauri::command]` |
| `src/stores/tracker.js` | состояние трекера во фронте, перевод статусов, подсчёт блокировок |
| `src/stores/mockBackend.js` | `mockIPC` с фикстурами для браузерного режима |
| `src/components/kanban/NewTaskModal.vue` | форма создания задачи |

---

### Задача 1: тулчейн Rust и каркас Tauri

**Файлы:**
- Создать: `src-tauri/` (генерируется), `src-tauri/tauri.conf.json`
- Изменить: `package.json`, `vite.config.js`, `.gitignore`

**Интерфейсы:**
- Отдаёт: работающие команды `npm run tauri dev` и `npm run tauri build`; окно приложения
  с текущим Vue-фронтом внутри.

- [ ] **Шаг 1: Установить Rust**

На машине его нет — это проверено, `rustc` не находится в PATH.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
```

Ожидается версия не ниже 1.77.2 — это минимум для Tauri 2. На macOS нужны ещё инструменты
командной строки Xcode; если `xcode-select -p` отвечает ошибкой, выполнить `xcode-select --install`.

- [ ] **Шаг 2: Поставить зависимости Tauri**

```bash
npm install --save-dev @tauri-apps/cli@^2
npm install @tauri-apps/api@^2
```

- [ ] **Шаг 3: Сгенерировать каркас неинтерактивно**

Флаг `--ci` обязателен: без него команда задаёт вопросы и подвисает.

```bash
npx tauri init --ci \
  -A smetana \
  -W Smetana \
  -D ../dist \
  -P http://localhost:5173 \
  --before-dev-command "npm run dev" \
  --before-build-command "npm run build"
```

- [ ] **Шаг 4: Поправить конфигурацию приложения**

В `src-tauri/tauri.conf.json` заменить сгенерированный идентификатор и размеры окна:

```json
{
  "productName": "smetana",
  "identifier": "com.invisor.smetana",
  "app": {
    "windows": [
      {
        "title": "Smetana",
        "width": 1440,
        "height": 900,
        "minWidth": 1024,
        "minHeight": 640
      }
    ]
  }
}
```

- [ ] **Шаг 5: Научить Vite сосуществовать с Tauri**

В `vite.config.js` добавить два поля к существующему объекту: Tauri сам ведёт вывод в терминале, а
сборщик не должен реагировать на изменения в Rust-каталоге.

```js
export default defineConfig({
  plugins: [vue()],
  // Tauri сам управляет выводом в терминале
  clearScreen: false,
  server: {
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] }
  },
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) }
  },
  build: { target: ['es2021', 'chrome100', 'safari15'] }
})
```

- [ ] **Шаг 6: Дополнить `.gitignore`**

```
src-tauri/target/
src-tauri/gen/
```

Каталог `src-tauri/binaries/` здесь намеренно не упоминается: в задаче 2 в нём появится
отслеживаемый файл лицензии, а перекрыть исключение целого каталога точечным `!` git не умеет.

- [ ] **Шаг 7: Проверить, что окно открывается**

```bash
npm run tauri dev
```

Ожидается: собирается Rust, открывается окно с трёхколоночным интерфейсом — тем же, что и в
браузере. Первая сборка занимает несколько минут, это нормально. Закрыть окно.

- [ ] **Шаг 8: Проверить, что браузерный режим не сломался**

```bash
npm run dev
```

Открыть `http://localhost:5173/?view=gallery` — галерея рендерится как раньше.

- [ ] **Шаг 9: Коммит**

```bash
git add -A
git commit -m "feat: каркас Tauri поверх существующего фронта"
```

---

### Задача 2: поставка bd вшитым бинарником

**Файлы:**
- Создать: `scripts/fetch-bd.mjs`
- Изменить: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`

**Интерфейсы:**
- Отдаёт: sidecar `bd`, доступный из Rust как `app.shell().sidecar("bd")`; каталог
  `src-tauri/binaries/bd-<triple>`, заполняемый скриптом и не попадающий в git.

- [ ] **Шаг 1: Написать скрипт загрузки**

Создать `scripts/fetch-bd.mjs`. Распаковка идёт системным `tar` — на macOS, Linux и Windows 10+ это
bsdtar, который понимает и `.tar.gz`, и `.zip`, поэтому новых зависимостей не появляется.

```js
#!/usr/bin/env node
/* Кладёт релизный бинарник bd в src-tauri/binaries под именем, которого ждёт Tauri:
   bd-<target-triple>[.exe]. Бинарник весит 128 МБ и в git не коммитится.

   Сборка bd из Homebrew непереносима — она линкуется на icu4c из /opt/homebrew.
   Официальный релиз зависит только от системных библиотек, поэтому берём именно его. */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const BD_VERSION = '1.1.2'
const BASE = `https://github.com/gastownhall/beads/releases/download/v${BD_VERSION}`

const ASSET_BY_TRIPLE = {
  'aarch64-apple-darwin': `beads_${BD_VERSION}_darwin_arm64.tar.gz`,
  'x86_64-apple-darwin': `beads_${BD_VERSION}_darwin_amd64.tar.gz`,
  'aarch64-unknown-linux-gnu': `beads_${BD_VERSION}_linux_arm64.tar.gz`,
  'x86_64-unknown-linux-gnu': `beads_${BD_VERSION}_linux_amd64.tar.gz`,
  'aarch64-pc-windows-msvc': `beads_${BD_VERSION}_windows_arm64.zip`,
  'x86_64-pc-windows-msvc': `beads_${BD_VERSION}_windows_amd64.zip`
}

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = join(root, 'src-tauri', 'binaries')

const hostTriple = () =>
  execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8' }).trim()

async function download(url, dest) {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`${url} → HTTP ${res.status}`)
  await writeFile(dest, Buffer.from(await res.arrayBuffer()))
}

async function checksums() {
  const res = await fetch(`${BASE}/checksums.txt`)
  if (!res.ok) throw new Error(`checksums.txt → HTTP ${res.status}`)
  const map = new Map()
  for (const line of (await res.text()).split('\n')) {
    const [sum, name] = line.trim().split(/\s+/)
    if (sum && name) map.set(name.replace(/^\*/, ''), sum)
  }
  return map
}

async function install(triple, sums) {
  const asset = ASSET_BY_TRIPLE[triple]
  if (!asset) throw new Error(`нет релиза bd для ${triple}`)

  const windows = triple.includes('windows')
  const target = join(outDir, windows ? `bd-${triple}.exe` : `bd-${triple}`)
  if (existsSync(target)) {
    console.log(`✓ ${triple} уже на месте`)
    return
  }

  const work = await mkdtemp(join(tmpdir(), 'fetch-bd-'))
  try {
    const archive = join(work, asset)
    console.log(`↓ ${asset}`)
    await download(`${BASE}/${asset}`, archive)

    const expected = sums.get(asset)
    if (!expected) throw new Error(`${asset} отсутствует в checksums.txt`)
    const actual = createHash('sha256').update(await readFile(archive)).digest('hex')
    if (actual !== expected) throw new Error(`sha256 не совпал: ${actual} вместо ${expected}`)

    execFileSync('tar', ['-xf', archive, '-C', work])
    await mkdir(outDir, { recursive: true })
    await copyFile(join(work, windows ? 'bd.exe' : 'bd'), target)
    await chmod(target, 0o755)
    console.log(`✓ ${target}`)
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

const triples = process.argv.includes('--all') ? Object.keys(ASSET_BY_TRIPLE) : [hostTriple()]
const sums = await checksums()
for (const triple of triples) await install(triple, sums)
```

- [ ] **Шаг 2: Запустить скрипт и убедиться, что бинарник рабочий**

```bash
node scripts/fetch-bd.mjs
ls -lh src-tauri/binaries/
./src-tauri/binaries/bd-$(rustc --print host-tuple) version
```

Ожидается: файл около 128 МБ и вывод `bd version 1.1.2 (...)`.

- [ ] **Шаг 3: Подключить скрипт к установке зависимостей**

В `package.json` добавить в `scripts`:

```json
"postinstall": "node scripts/fetch-bd.mjs",
"fetch-bd": "node scripts/fetch-bd.mjs"
```

- [ ] **Шаг 4: Объявить sidecar в конфигурации Tauri**

В `src-tauri/tauri.conf.json`, в объект `bundle`:

```json
"externalBin": ["binaries/bd"],
"resources": ["binaries/LICENSE-bd"]
```

Лицензия bd — MIT, она обязывает приложить текст и копирайт. Сохранить его рядом:

```bash
mkdir -p src-tauri/binaries
curl -sL "https://raw.githubusercontent.com/gastownhall/beads/v1.1.2/LICENSE" \
  -o src-tauri/binaries/LICENSE-bd
```

Файл лицензии, в отличие от бинарника, коммитится — дописать в `.gitignore` две строки. Именно
`binaries/*`, а не `binaries/`: исключение целого каталога точечным `!` не перекрывается.

```
src-tauri/binaries/*
!src-tauri/binaries/LICENSE-bd
```

- [ ] **Шаг 5: Подключить плагин shell в Rust**

В `src-tauri/Cargo.toml`, секция `[dependencies]`:

```toml
tauri-plugin-shell = "2"
serde_json = "1"
thiserror = "2"
notify = "8"
tokio = { version = "1", features = ["sync", "time", "rt", "macros"] }
```

В `src-tauri/src/lib.rs` зарегистрировать плагин:

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Разрешение `shell:allow-execute` в capabilities не добавляем намеренно.** Оно нужно только для
вызова sidecar из JavaScript, а у нас bd вызывает исключительно Rust. Выдать его — значит разрешить
фронту запускать bd с произвольными аргументами без всякой на то надобности.

- [ ] **Шаг 6: Проверить, что sidecar виден из Rust**

Временно добавить в `run()` перед `.run(...)`:

```rust
        .setup(|app| {
            use tauri_plugin_shell::ShellExt;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let out = handle.shell().sidecar("bd").unwrap().args(["version"]).output().await;
                println!("bd → {:?}", out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()));
            });
            Ok(())
        })
```

```bash
npm run tauri dev
```

Ожидается строка вида `bd → Ok("bd version 1.1.2 (...)")` в терминале. После проверки блок
`.setup(...)` удалить — в задаче 7 появится настоящий.

- [ ] **Шаг 7: Коммит**

```bash
git add -A
git commit -m "feat: bd едет в бандле как sidecar"
```

---

### Задача 3: модель данных и разбор вывода bd

**Файлы:**
- Создать: `src-tauri/src/tracker/mod.rs`, `src-tauri/src/tracker/model.rs`,
  `src-tauri/src/tracker/bd.rs`
- Изменить: `src-tauri/src/lib.rs`

**Интерфейсы:**
- Отдаёт: `model::{Issue, Dependency, ColumnDef, Delta, Snapshot, NewIssue, IssuePatch, TrackerError}`;
  `bd::{parse_issues, parse_columns, parse_version, create_args, update_args}`.

Формы разбираемых данных проверены на живом bd 1.1.2, а не взяты из документации:
пустые поля bd опускает, `bd create` отдаёт объект, а `bd update` и `bd close` — массив.

- [ ] **Шаг 1: Написать падающие тесты разбора**

Создать `src-tauri/src/tracker/bd.rs` сразу с тестами внизу файла:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Так выглядит выдача bd list --json: пустые поля отсутствуют целиком.
    const LIST: &str = r#"[
      {"id":"smetana-29j","title":"Живая синхронизация","status":"open","priority":1,
       "issue_type":"feature","created_at":"2026-07-30T21:31:27Z","updated_at":"2026-07-30T21:31:27Z",
       "dependency_count":0,"dependent_count":0,"comment_count":0},
      {"id":"smetana-3km","title":"проверка контракта","status":"open","priority":2,
       "issue_type":"task","assignee":"flexo","labels":["alpha"],"parent":"smetana-29j",
       "updated_at":"2026-07-31T00:58:55Z",
       "dependencies":[
         {"issue_id":"smetana-3km","depends_on_id":"smetana-1or","type":"blocks",
          "created_at":"2026-07-31T00:58:55Z","created_by":"flexo","metadata":"{}"},
         {"issue_id":"smetana-3km","depends_on_id":"smetana-29j","type":"parent-child",
          "created_at":"2026-07-31T00:58:55Z","created_by":"flexo","metadata":"{}"}]}
    ]"#;

    /// bd create отдаёт объект, а не массив.
    const CREATED: &str = r#"{"id":"smetana-3km","title":"проверка контракта","status":"open",
      "priority":2,"issue_type":"task","updated_at":"2026-07-30T21:57:07Z"}"#;

    const STATUSES: &str = r#"{"built_in_statuses":[
        {"category":"active","description":"Available to work","icon":"○","name":"open"},
        {"category":"done","description":"Completed","icon":"✓","name":"closed"},
        {"category":"wip","description":"Actively being worked on","icon":"◐","name":"in_progress"}],
      "custom_statuses":[{"category":"wip","name":"awaiting-review"}],
      "schema_version":1}"#;

    #[test]
    fn разбирает_массив_задач() {
        let issues = parse_issues(LIST).unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].id, "smetana-29j");
        assert_eq!(issues[0].assignee, None);
        assert!(issues[0].labels.is_empty());
    }

    #[test]
    fn разбирает_одиночный_объект() {
        let issues = parse_issues(CREATED).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "smetana-3km");
    }

    #[test]
    fn сохраняет_тип_зависимости() {
        let issues = parse_issues(LIST).unwrap();
        let kinds: Vec<&str> = issues[1].dependencies.iter().map(|d| d.kind.as_str()).collect();
        assert_eq!(kinds, vec!["blocks", "parent-child"]);
    }

    #[test]
    fn пропускает_баннер_перед_json() {
        let issues = parse_issues("warning: beads.role not configured\n[]").unwrap();
        assert!(issues.is_empty());
    }

    #[test]
    fn колонки_идут_встроенные_и_кастомные_в_порядке_категорий() {
        let cols = parse_columns(STATUSES).unwrap();
        let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["open", "in_progress", "awaiting-review", "closed"]);
    }

    #[test]
    fn достаёт_версию() {
        assert_eq!(parse_version("bd version 1.1.2 (20e493e5)").as_deref(), Some("1.1.2"));
        assert_eq!(parse_version("чепуха"), None);
    }

    #[test]
    fn аргументы_обновления_содержат_только_заданные_поля() {
        let patch = IssuePatch { status: Some("in_progress".into()), title: Some("новое".into()),
            ..Default::default() };
        assert_eq!(update_args("smetana-1", &patch),
            vec!["update", "smetana-1", "--json", "-s", "in_progress", "--title", "новое"]);
    }
}
```

- [ ] **Шаг 2: Запустить тесты и убедиться, что они падают**

```bash
cd src-tauri && cargo test tracker::bd
```

Ожидается: ошибка компиляции — `parse_issues`, `parse_columns` и остальное ещё не существуют.

- [ ] **Шаг 3: Описать модель**

Создать `src-tauri/src/tracker/model.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Ребро графа зависимостей. bd отдаёт у задачи только исходящие связи:
/// issue_id зависит от depends_on_id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub issue_id: String,
    pub depends_on_id: String,
    /// "blocks", "parent-child", "related", "discovered-from"
    #[serde(rename = "type")]
    pub kind: String,
}

/// Задача в том виде, в каком её отдаёт bd. Пустые поля bd опускает целиком,
/// поэтому всё необязательное — Option или коллекция со значением по умолчанию.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated_at: String,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

/// Колонка доски. Из bd берём только имя и категорию: глиф и цвет
/// принадлежат status.js, свои иконки bd мы намеренно игнорируем.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    /// "active" | "wip" | "frozen" | "done"
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub generation: u64,
    pub columns: Vec<ColumnDef>,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Delta {
    pub generation: u64,
    pub upserted: Vec<Issue>,
    pub removed: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<ColumnDef>>,
}

impl Delta {
    pub fn is_empty(&self) -> bool {
        self.upserted.is_empty() && self.removed.is_empty() && self.columns.is_none()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewIssue {
    pub title: String,
    pub issue_type: String,
    pub priority: i64,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IssuePatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub issue_type: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub add_labels: Vec<String>,
    #[serde(default)]
    pub remove_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    Ok,
    NotABeadsRepo,
    BdVersionMismatch,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Health {
    pub state: HealthState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("bd завершился с кодом {code}: {stderr}")]
    Command { code: i32, stderr: String },
    #[error("в выводе bd нет JSON")]
    NoJson,
    #[error("не удалось разобрать вывод bd: {0}")]
    Parse(String),
    #[error("не удалось запустить bd: {0}")]
    Spawn(String),
    #[error("bd вернул пустой результат")]
    Empty,
}

// Tauri требует, чтобы ошибка команды умела сериализоваться.
impl Serialize for TrackerError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
```

- [ ] **Шаг 4: Написать разбор и сборку аргументов**

В начало `src-tauri/src/tracker/bd.rs`, перед блоком тестов:

```rust
use super::model::{ColumnDef, IssuePatch, Issue, NewIssue, TrackerError};

/// Порядок колонок задают категории bd: сначала доступное, потом в работе,
/// потом отложенное, потом завершённое.
fn category_rank(category: &str) -> u8 {
    match category {
        "active" => 0,
        "wip" => 1,
        "frozen" => 2,
        "done" => 3,
        _ => 4,
    }
}

/// Предупреждения bd уходят в stderr, но полагаться на это целиком не стоит:
/// отрезаем всё до первой скобки.
fn slice_json(stdout: &str) -> Result<&str, TrackerError> {
    stdout
        .find(['[', '{'])
        .map(|i| &stdout[i..])
        .ok_or(TrackerError::NoJson)
}

/// bd create отдаёт объект, а update и close — массив, потому что принимают
/// несколько идентификаторов. Приводим обе формы к вектору.
pub fn parse_issues(stdout: &str) -> Result<Vec<Issue>, TrackerError> {
    let value: serde_json::Value =
        serde_json::from_str(slice_json(stdout)?).map_err(|e| TrackerError::Parse(e.to_string()))?;
    match value {
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            let wrapped = if value.is_array() {
                value
            } else {
                serde_json::Value::Array(vec![value])
            };
            serde_json::from_value(wrapped).map_err(|e| TrackerError::Parse(e.to_string()))
        }
        _ => Err(TrackerError::Parse("ожидался объект или массив".into())),
    }
}

pub fn parse_columns(stdout: &str) -> Result<Vec<ColumnDef>, TrackerError> {
    #[derive(serde::Deserialize)]
    struct Out {
        #[serde(default)]
        built_in_statuses: Vec<ColumnDef>,
        #[serde(default)]
        custom_statuses: Vec<ColumnDef>,
    }
    let out: Out = serde_json::from_str(slice_json(stdout)?)
        .map_err(|e| TrackerError::Parse(e.to_string()))?;
    let mut columns = out.built_in_statuses;
    columns.extend(out.custom_statuses);
    columns.sort_by_key(|c| category_rank(&c.category));
    Ok(columns)
}

pub fn parse_version(stdout: &str) -> Option<String> {
    stdout
        .split_whitespace()
        .skip_while(|w| *w != "version")
        .nth(1)
        .map(str::to_string)
}

pub fn create_args(new: &NewIssue) -> Vec<String> {
    let mut args = vec![
        "create".to_string(),
        new.title.clone(),
        "--json".to_string(),
        "-t".to_string(),
        new.issue_type.clone(),
        "-p".to_string(),
        new.priority.to_string(),
    ];
    if let Some(description) = &new.description {
        args.push("-d".into());
        args.push(description.clone());
    }
    args
}

pub fn update_args(id: &str, patch: &IssuePatch) -> Vec<String> {
    let mut args = vec!["update".to_string(), id.to_string(), "--json".to_string()];
    let mut push = |flag: &str, value: String| {
        args.push(flag.to_string());
        args.push(value);
    };
    if let Some(v) = &patch.status {
        push("-s", v.clone());
    }
    if let Some(v) = &patch.title {
        push("--title", v.clone());
    }
    if let Some(v) = &patch.description {
        push("-d", v.clone());
    }
    if let Some(v) = &patch.issue_type {
        push("-t", v.clone());
    }
    if let Some(v) = patch.priority {
        push("-p", v.to_string());
    }
    if let Some(v) = &patch.assignee {
        push("-a", v.clone());
    }
    for label in &patch.add_labels {
        push("--add-label", label.clone());
    }
    for label in &patch.remove_labels {
        push("--remove-label", label.clone());
    }
    args
}
```

Порядок флагов в `update_args` зафиксирован тестом, поэтому менять его местами нельзя без правки
теста.

- [ ] **Шаг 5: Подключить модуль**

Создать `src-tauri/src/tracker/mod.rs`:

```rust
pub mod bd;
pub mod model;
```

В `src-tauri/src/lib.rs` добавить первой строкой:

```rust
mod tracker;
```

- [ ] **Шаг 6: Запустить тесты**

```bash
cd src-tauri && cargo test tracker::bd
```

Ожидается: все семь тестов проходят.

- [ ] **Шаг 7: Коммит**

```bash
git add -A
git commit -m "feat: модель трекера и разбор вывода bd"
```

---

### Задача 4: запуск bd

**Файлы:**
- Изменить: `src-tauri/src/tracker/bd.rs`

**Интерфейсы:**
- Потребляет: `model::*`, `bd::{create_args, update_args, parse_issues, parse_columns, parse_version}`
- Отдаёт: `bd::Bd` с методами `columns`, `list_all`, `list_updated_after`, `create`, `update`,
  `close`, `reopen`, `version` — все возвращают `Result<_, TrackerError>`.

- [ ] **Шаг 1: Написать раннер**

Добавить в `src-tauri/src/tracker/bd.rs` перед блоком тестов:

```rust
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

/// Обёртка над вшитым бинарником bd. Единственное место, которое знает,
/// как выглядят аргументы CLI.
#[derive(Clone)]
pub struct Bd {
    app: AppHandle,
    cwd: PathBuf,
}

impl Bd {
    pub fn new(app: AppHandle, cwd: PathBuf) -> Self {
        Self { app, cwd }
    }

    /// Ошибкой считается только ненулевой код возврата. Предупреждения bd
    /// ("dolt auto-push failed", "beads.role not configured") идут в stderr
    /// постоянно и ошибкой не являются.
    async fn run(&self, args: Vec<String>) -> Result<String, TrackerError> {
        let output = self
            .app
            .shell()
            .sidecar("bd")
            .map_err(|e| TrackerError::Spawn(e.to_string()))?
            .current_dir(self.cwd.clone())
            .args(args)
            .output()
            .await
            .map_err(|e| TrackerError::Spawn(e.to_string()))?;

        if !output.status.success() {
            return Err(TrackerError::Command {
                code: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn one(&self, args: Vec<String>) -> Result<Issue, TrackerError> {
        parse_issues(&self.run(args).await?)?
            .into_iter()
            .next()
            .ok_or(TrackerError::Empty)
    }

    pub async fn version(&self) -> Result<Option<String>, TrackerError> {
        Ok(parse_version(&self.run(vec!["version".into()]).await?))
    }

    pub async fn columns(&self) -> Result<Vec<ColumnDef>, TrackerError> {
        parse_columns(&self.run(vec!["statuses".into(), "--json".into()]).await?)
    }

    /// -n 0 обязателен: по умолчанию bd list отдаёт только 50 записей.
    pub async fn list_all(&self) -> Result<Vec<Issue>, TrackerError> {
        parse_issues(
            &self
                .run(vec![
                    "list".into(),
                    "--all".into(),
                    "-n".into(),
                    "0".into(),
                    "--json".into(),
                ])
                .await?,
        )
    }

    pub async fn list_updated_after(&self, since: &str) -> Result<Vec<Issue>, TrackerError> {
        parse_issues(
            &self
                .run(vec![
                    "list".into(),
                    "--all".into(),
                    "-n".into(),
                    "0".into(),
                    "--updated-after".into(),
                    since.to_string(),
                    "--json".into(),
                ])
                .await?,
        )
    }

    pub async fn create(&self, new: &NewIssue) -> Result<Issue, TrackerError> {
        self.one(create_args(new)).await
    }

    pub async fn update(&self, id: &str, patch: &IssuePatch) -> Result<Issue, TrackerError> {
        self.one(update_args(id, patch)).await
    }

    pub async fn close(&self, id: &str, reason: Option<&str>) -> Result<Issue, TrackerError> {
        let mut args = vec!["close".to_string(), id.to_string(), "--json".to_string()];
        if let Some(reason) = reason {
            args.push("-r".into());
            args.push(reason.to_string());
        }
        self.one(args).await
    }

    pub async fn reopen(&self, id: &str) -> Result<Issue, TrackerError> {
        self.one(vec!["reopen".into(), id.to_string(), "--json".into()])
            .await
    }
}
```

- [ ] **Шаг 2: Проверить, что всё компилируется и тесты не сломались**

```bash
cd src-tauri && cargo test tracker::bd
```

Ожидается: те же семь тестов проходят, предупреждений о неиспользуемом коде может быть много —
это нормально, `Bd` подключится в задаче 7.

- [ ] **Шаг 3: Коммит**

```bash
git add -A
git commit -m "feat: запуск bd через sidecar"
```

---

### Задача 5: снимок трекера и вычисление дельты

**Файлы:**
- Создать: `src-tauri/src/tracker/store.rs`
- Изменить: `src-tauri/src/tracker/mod.rs`

**Интерфейсы:**
- Отдаёт: `store::Store` с методами `snapshot`, `set_columns`, `apply_incremental`, `apply_full`,
  `last_seen`, `generation`.

Логика чистая: ни процессов, ни Tauri, поэтому покрывается тестами целиком.

- [ ] **Шаг 1: Написать падающие тесты**

Создать `src-tauri/src/tracker/store.rs` с тестами:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::model::Issue;

    fn issue(id: &str, status: &str, updated: &str) -> Issue {
        Issue {
            id: id.into(),
            title: format!("задача {id}"),
            status: status.into(),
            updated_at: updated.into(),
            priority: None,
            issue_type: None,
            assignee: None,
            parent: None,
            labels: vec![],
            dependencies: vec![],
        }
    }

    #[test]
    fn новая_задача_попадает_в_дельту() {
        let mut store = Store::default();
        let delta = store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert_eq!(delta.upserted.len(), 1);
        assert_eq!(delta.generation, 1);
    }

    #[test]
    fn неизменившаяся_задача_дельту_не_порождает() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert!(delta.is_empty());
        assert_eq!(store.generation(), 1, "поколение растёт только при непустой дельте");
    }

    #[test]
    fn смена_статуса_попадает_в_дельту() {
        let mut store = Store::default();
        store.apply_incremental(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        let delta = store.apply_incremental(vec![issue("a", "closed", "2026-07-31T00:00:02Z")]);
        assert_eq!(delta.upserted.len(), 1);
        assert_eq!(delta.upserted[0].status, "closed");
    }

    #[test]
    fn инкремент_не_удаляет_отсутствующее() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.apply_incremental(vec![issue("a", "closed", "2026-07-31T00:00:02Z")]);
        assert!(delta.removed.is_empty());
    }

    #[test]
    fn полная_сверка_убирает_пропавшее() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:01Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        let delta = store.apply_full(vec![issue("a", "open", "2026-07-31T00:00:01Z")]);
        assert_eq!(delta.removed, vec!["b".to_string()]);
    }

    #[test]
    fn запоминает_наибольшую_метку_времени() {
        let mut store = Store::default();
        store.apply_incremental(vec![
            issue("a", "open", "2026-07-31T00:00:05Z"),
            issue("b", "open", "2026-07-31T00:00:01Z"),
        ]);
        assert_eq!(store.last_seen(), "2026-07-31T00:00:05Z");
    }

    #[test]
    fn смена_набора_колонок_попадает_в_дельту() {
        let mut store = Store::default();
        assert!(store.set_columns(vec![ColumnDef { name: "open".into(), category: "active".into() }]));
        assert!(!store.set_columns(vec![ColumnDef { name: "open".into(), category: "active".into() }]));
    }
}
```

- [ ] **Шаг 2: Запустить тесты и убедиться, что они падают**

```bash
cd src-tauri && cargo test tracker::store
```

Ожидается: ошибка компиляции — `Store` ещё не существует.

- [ ] **Шаг 3: Написать хранилище**

В начало `src-tauri/src/tracker/store.rs`:

```rust
use std::collections::BTreeMap;

use super::model::{ColumnDef, Delta, Issue, Snapshot};

/// Снимок трекера в памяти. Владеет им единственный воркер, поэтому
/// синхронизация здесь не нужна.
#[derive(Default)]
pub struct Store {
    issues: BTreeMap<String, Issue>,
    columns: Vec<ColumnDef>,
    generation: u64,
    last_seen: String,
}

impl Store {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Метка времени для следующей инкрементальной догрузки.
    pub fn last_seen(&self) -> &str {
        &self.last_seen
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            generation: self.generation,
            columns: self.columns.clone(),
            issues: self.issues.values().cloned().collect(),
        }
    }

    /// Возвращает true, если набор колонок действительно изменился.
    pub fn set_columns(&mut self, columns: Vec<ColumnDef>) -> bool {
        if self.columns == columns {
            return false;
        }
        self.columns = columns;
        true
    }

    pub fn apply_incremental(&mut self, fetched: Vec<Issue>) -> Delta {
        self.apply(fetched, false)
    }

    /// Полная сверка: всё, чего нет в выборке, считается удалённым.
    /// Инкремент так делать не может — он по определению видит не всё.
    pub fn apply_full(&mut self, fetched: Vec<Issue>) -> Delta {
        self.apply(fetched, true)
    }

    fn apply(&mut self, fetched: Vec<Issue>, full: bool) -> Delta {
        let mut delta = Delta::default();

        if full {
            let seen: std::collections::BTreeSet<&String> = fetched.iter().map(|i| &i.id).collect();
            delta.removed = self
                .issues
                .keys()
                .filter(|id| !seen.contains(id))
                .cloned()
                .collect();
            for id in &delta.removed {
                self.issues.remove(id);
            }
        }

        for issue in fetched {
            if issue.updated_at > self.last_seen {
                self.last_seen = issue.updated_at.clone();
            }
            if self.issues.get(&issue.id) != Some(&issue) {
                self.issues.insert(issue.id.clone(), issue.clone());
                delta.upserted.push(issue);
            }
        }

        if !delta.is_empty() {
            self.generation += 1;
            delta.generation = self.generation;
        }
        delta
    }

    /// Кладёт результат собственной записи, не дожидаясь watcher.
    pub fn upsert_one(&mut self, issue: Issue) -> Delta {
        self.apply_incremental(vec![issue])
    }

    pub fn columns_delta(&mut self) -> Delta {
        self.generation += 1;
        Delta {
            generation: self.generation,
            columns: Some(self.columns.clone()),
            ..Default::default()
        }
    }
}
```

- [ ] **Шаг 4: Подключить модуль**

В `src-tauri/src/tracker/mod.rs` добавить:

```rust
pub mod store;
```

- [ ] **Шаг 5: Запустить тесты**

```bash
cd src-tauri && cargo test tracker::store
```

Ожидается: все семь тестов проходят.

- [ ] **Шаг 6: Коммит**

```bash
git add -A
git commit -m "feat: снимок трекера и вычисление дельты"
```

---

### Задача 6: слежение за каталогом .beads

**Файлы:**
- Создать: `src-tauri/src/tracker/watcher.rs`
- Изменить: `src-tauri/src/tracker/mod.rs`

**Интерфейсы:**
- Отдаёт: `watcher::is_relevant(path: &Path) -> bool` и
  `watcher::spawn(beads_dir: PathBuf, tx: tokio::sync::mpsc::Sender<()>) -> notify::Result<RecommendedWatcher>`.
  Возвращённый watcher нужно держать живым, иначе слежение прекращается.

Проверено на живом bd: любая запись трогает `.beads/embeddeddolt/<db>/.dolt/noms/manifest`,
`journal.idx` и `.beads/last-touched`.

- [ ] **Шаг 1: Написать падающие тесты фильтра**

Создать `src-tauri/src/tracker/watcher.rs` с тестами:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ловит_запись_dolt() {
        assert!(is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/noms/manifest"
        )));
        assert!(is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/noms/journal.idx"
        )));
    }

    #[test]
    fn ловит_last_touched() {
        assert!(is_relevant(Path::new("/p/.beads/last-touched")));
    }

    #[test]
    fn игнорирует_шум() {
        assert!(!is_relevant(Path::new("/p/.beads/config.yaml")));
        assert!(!is_relevant(Path::new("/p/.beads/backup/LOCK")));
        assert!(!is_relevant(Path::new(
            "/p/.beads/embeddeddolt/smetana/.dolt/git-remote-cache/x/repo.git/config"
        )));
    }
}
```

- [ ] **Шаг 2: Запустить тесты и убедиться, что они падают**

```bash
cd src-tauri && cargo test tracker::watcher
```

Ожидается: ошибка компиляции — `is_relevant` не существует.

- [ ] **Шаг 3: Написать наблюдатель**

В начало `src-tauri/src/tracker/watcher.rs`:

```rust
use std::path::{Path, PathBuf};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::Sender;

/// Значимых путей ровно три. Всё остальное в .beads — конфиги, бэкапы и
/// кэш git-ремоута — шумит, но к содержимому трекера отношения не имеет.
pub fn is_relevant(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let in_noms = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some("noms");
    (in_noms && (name == "manifest" || name == "journal.idx")) || name == "last-touched"
}

/// Возвращённый watcher нужно держать живым: при его уничтожении слежение
/// прекращается молча.
pub fn spawn(beads_dir: PathBuf, tx: Sender<()>) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.paths.iter().any(|p| is_relevant(p)) {
            // Схлопывание частых событий делает воркер; здесь достаточно
            // не блокироваться, если очередь уже полна.
            let _ = tx.try_send(());
        }
    })?;
    watcher.watch(&beads_dir, RecursiveMode::Recursive)?;
    Ok(watcher)
}
```

- [ ] **Шаг 4: Подключить модуль**

В `src-tauri/src/tracker/mod.rs` добавить:

```rust
pub mod watcher;
```

- [ ] **Шаг 5: Запустить тесты**

```bash
cd src-tauri && cargo test tracker::watcher
```

Ожидается: все три теста проходят.

- [ ] **Шаг 6: Коммит**

```bash
git add -A
git commit -m "feat: слежение за каталогом .beads"
```

---

### Задача 7: воркер и команды Tauri

**Файлы:**
- Создать: `src-tauri/src/tracker/service.rs`, `src-tauri/src/tracker/commands.rs`
- Изменить: `src-tauri/src/tracker/mod.rs`, `src-tauri/src/lib.rs`

**Интерфейсы:**
- Потребляет: `Bd`, `Store`, `watcher::spawn`
- Отдаёт фронту команды `tracker_snapshot`, `tracker_resync`, `tracker_create`, `tracker_update`,
  `tracker_close`, `tracker_reopen` и события `tracker:delta`, `tracker:health`.

- [ ] **Шаг 1: Написать воркер**

Создать `src-tauri/src/tracker/service.rs`:

```rust
use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::bd::Bd;
use super::model::{Health, HealthState, IssuePatch, Issue, NewIssue, Snapshot, TrackerError};
use super::store::Store;
use super::watcher;

/// Ожидаемая версия bd. Держится в одной строке с BD_VERSION из
/// scripts/fetch-bd.mjs — расхождение видно в health.
const EXPECTED_BD_VERSION: &str = "1.1.2";
/// Записи прилетают пачками; ждём, пока поток утихнет.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// Страховочная полная сверка: ловит удаления и пропущенные события.
const FULL_RESYNC: Duration = Duration::from_secs(60);
/// Запас на округление updated_at до секунды. Пропуск дороже повтора,
/// а дифф идемпотентен.
const OVERLAP_SECONDS: i64 = 5;

pub enum Request {
    Snapshot(oneshot::Sender<Snapshot>),
    Resync(oneshot::Sender<Result<Snapshot, TrackerError>>),
    Create(NewIssue, oneshot::Sender<Result<Issue, TrackerError>>),
    Update(String, IssuePatch, oneshot::Sender<Result<Issue, TrackerError>>),
    Close(String, Option<String>, oneshot::Sender<Result<Issue, TrackerError>>),
    Reopen(String, oneshot::Sender<Result<Issue, TrackerError>>),
}

#[derive(Clone)]
pub struct TrackerHandle(pub mpsc::Sender<Request>);

/// Единственное место с изменяемым состоянием — и оно однопоточное.
/// Вызов bd стоит около двух секунд, поэтому очередь запросов даёт
/// понятный порядок вместо непредсказуемых блокировок на мьютексе.
pub fn start(app: AppHandle, project_dir: PathBuf) -> TrackerHandle {
    let (tx_req, mut rx_req) = mpsc::channel::<Request>(32);
    let (tx_tick, mut rx_tick) = mpsc::channel::<()>(1);

    tauri::async_runtime::spawn(async move {
        let beads_dir = project_dir.join(".beads");
        if !beads_dir.is_dir() {
            emit_health(&app, HealthState::NotABeadsRepo, Some(format!(
                "в {} нет каталога .beads", project_dir.display()
            )));
            return;
        }

        let bd = Bd::new(app.clone(), project_dir.clone());
        let mut store = Store::default();

        match bd.version().await {
            Ok(Some(version)) if version == EXPECTED_BD_VERSION => {
                emit_health(&app, HealthState::Ok, None)
            }
            Ok(other) => emit_health(&app, HealthState::BdVersionMismatch, Some(format!(
                "ожидалась версия bd {EXPECTED_BD_VERSION}, получена {other:?}"
            ))),
            Err(e) => emit_health(&app, HealthState::Error, Some(e.to_string())),
        }

        // Держим watcher живым до конца работы воркера.
        let _watcher = match watcher::spawn(beads_dir, tx_tick.clone()) {
            Ok(w) => Some(w),
            Err(e) => {
                emit_health(&app, HealthState::Error, Some(format!(
                    "не удалось следить за .beads: {e}; остаётся только периодическая сверка"
                )));
                None
            }
        };

        full_sync(&app, &bd, &mut store).await;

        let mut ticker = tokio::time::interval(FULL_RESYNC);
        ticker.tick().await; // первый срабатывает мгновенно

        loop {
            tokio::select! {
                Some(request) = rx_req.recv() => {
                    handle(&app, &bd, &mut store, request).await;
                }
                Some(()) = rx_tick.recv() => {
                    tokio::time::sleep(DEBOUNCE).await;
                    while rx_tick.try_recv().is_ok() {}
                    incremental_sync(&app, &bd, &mut store).await;
                }
                _ = ticker.tick() => {
                    full_sync(&app, &bd, &mut store).await;
                }
                else => break,
            }
        }
    });

    TrackerHandle(tx_req)
}

fn emit_health(app: &AppHandle, state: HealthState, message: Option<String>) {
    let _ = app.emit("tracker:health", Health { state, message });
}

fn emit_delta(app: &AppHandle, delta: super::model::Delta) {
    if !delta.is_empty() {
        let _ = app.emit("tracker:delta", delta);
    }
}

/// updated_at округляется до секунды, поэтому просим с запасом.
fn since_with_overlap(last_seen: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(last_seen) {
        Ok(t) => (t - chrono::Duration::seconds(OVERLAP_SECONDS))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

async fn full_sync(app: &AppHandle, bd: &Bd, store: &mut Store) {
    match bd.columns().await {
        Ok(columns) => {
            if store.set_columns(columns) {
                emit_delta(app, store.columns_delta());
            }
        }
        Err(e) => emit_health(app, HealthState::Error, Some(e.to_string())),
    }
    match bd.list_all().await {
        Ok(issues) => emit_delta(app, store.apply_full(issues)),
        Err(e) => emit_health(app, HealthState::Error, Some(e.to_string())),
    }
}

async fn incremental_sync(app: &AppHandle, bd: &Bd, store: &mut Store) {
    let since = since_with_overlap(store.last_seen());
    match bd.list_updated_after(&since).await {
        Ok(issues) => emit_delta(app, store.apply_incremental(issues)),
        Err(e) => emit_health(app, HealthState::Error, Some(e.to_string())),
    }
}

async fn handle(app: &AppHandle, bd: &Bd, store: &mut Store, request: Request) {
    match request {
        Request::Snapshot(reply) => {
            let _ = reply.send(store.snapshot());
        }
        Request::Resync(reply) => {
            full_sync(app, bd, store).await;
            let _ = reply.send(Ok(store.snapshot()));
        }
        Request::Create(new, reply) => {
            let _ = reply.send(finish(app, store, bd.create(&new).await));
        }
        Request::Update(id, patch, reply) => {
            let _ = reply.send(finish(app, store, bd.update(&id, &patch).await));
        }
        Request::Close(id, reason, reply) => {
            let _ = reply.send(finish(app, store, bd.close(&id, reason.as_deref()).await));
        }
        Request::Reopen(id, reply) => {
            let _ = reply.send(finish(app, store, bd.reopen(&id).await));
        }
    }
}

/// Результат собственной записи кладём в снимок сразу, не дожидаясь watcher:
/// пришедший следом тик даст пустой дифф.
fn finish(
    app: &AppHandle,
    store: &mut Store,
    result: Result<Issue, TrackerError>,
) -> Result<Issue, TrackerError> {
    if let Ok(issue) = &result {
        emit_delta(app, store.upsert_one(issue.clone()));
    }
    result
}
```

Добавить в `src-tauri/Cargo.toml`:

```toml
chrono = "0.4"
```

- [ ] **Шаг 2: Написать команды**

Создать `src-tauri/src/tracker/commands.rs`:

```rust
use tauri::State;
use tokio::sync::oneshot;

use super::model::{IssuePatch, Issue, NewIssue, Snapshot, TrackerError};
use super::service::{Request, TrackerHandle};

/// Команды намеренно тонкие: всё, что они делают, — кладут запрос в очередь
/// воркера и ждут ответ.
async fn ask<T>(
    handle: &TrackerHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, TrackerError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| TrackerError::Spawn("воркер трекера не запущен".into()))?;
    rx.await
        .map_err(|_| TrackerError::Spawn("воркер трекера не ответил".into()))
}

#[tauri::command]
pub async fn tracker_snapshot(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Snapshot).await
}

#[tauri::command]
pub async fn tracker_resync(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Resync).await?
}

#[tauri::command]
pub async fn tracker_create(
    handle: State<'_, TrackerHandle>,
    issue: NewIssue,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Create(issue, tx)).await?
}

#[tauri::command]
pub async fn tracker_update(
    handle: State<'_, TrackerHandle>,
    id: String,
    patch: IssuePatch,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Update(id, patch, tx)).await?
}

#[tauri::command]
pub async fn tracker_close(
    handle: State<'_, TrackerHandle>,
    id: String,
    reason: Option<String>,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Close(id, reason, tx)).await?
}

#[tauri::command]
pub async fn tracker_reopen(
    handle: State<'_, TrackerHandle>,
    id: String,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Reopen(id, tx)).await?
}
```

- [ ] **Шаг 3: Собрать всё в приложении**

В `src-tauri/src/tracker/mod.rs`:

```rust
pub mod bd;
pub mod commands;
pub mod model;
pub mod service;
pub mod store;
pub mod watcher;
```

В `src-tauri/src/lib.rs`:

```rust
mod tracker;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Проект один — каталог, из которого запущено приложение.
            // Выбор каталога появится позже.
            let project_dir = std::env::current_dir()?;
            let handle = tracker::service::start(app.handle().clone(), project_dir);
            app.manage(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tracker::commands::tracker_snapshot,
            tracker::commands::tracker_resync,
            tracker::commands::tracker_create,
            tracker::commands::tracker_update,
            tracker::commands::tracker_close,
            tracker::commands::tracker_reopen,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Шаг 4: Проверить сборку и тесты**

```bash
cd src-tauri && cargo test
```

Ожидается: все семнадцать тестов из задач 3, 5 и 6 проходят, сборка без ошибок.

- [ ] **Шаг 5: Проверить работу вживую**

```bash
npm run tauri dev
```

В окне приложения открыть инструменты разработчика (правый клик → Inspect) и выполнить в консоли:

```js
await window.__TAURI_INTERNALS__.invoke('tracker_snapshot')
```

Ожидается объект с `columns` (не менее шести встроенных статусов), `issues` (как минимум
`smetana-29j`) и `generation`.

Затем, не закрывая окно, в терминале:

```bash
bd create "проверка живого обновления" -t task -p 3
```

В консоли окна заранее подписаться:

```js
window.__TAURI__.event.listen('tracker:delta', (e) => console.log('дельта', e.payload))
```

Ожидается: в течение примерно секунды приходит дельта с новой задачей.

- [ ] **Шаг 6: Коммит**

```bash
git add -A
git commit -m "feat: воркер трекера и команды Tauri"
```

---

### Задача 8: доска на реальных данных

**Файлы:**
- Создать: `src/stores/tracker.js`, `src/stores/mockBackend.js`
- Изменить: `src/main.js`, `src/views/DesktopApp.vue`

**Интерфейсы:**
- Потребляет: команды и события из задачи 7
- Отдаёт: `tracker.js` экспортирует `initTracker()`, `trackerState`, `boardColumns`, `issueById`,
  `toUiStatus()`; `mockBackend.js` экспортирует `installMockBackend()`.

- [ ] **Шаг 1: Написать состояние трекера**

Создать `src/stores/tracker.js`:

```js
/* Состояние трекера во фронте. Компоненты знают только это хранилище;
   про Tauri знает лишь оно само. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/* bd и дизайн-система называют одно и то же по-разному. RESERVED в status.js —
   ready/running/done, в bd — open/in_progress/closed. Пересечение только по
   blocked. Без перевода карточки потеряли бы глифы и уехали бы в
   генерируемые хэш-цвета. Всё остальное, включая кастомные статусы,
   уходит в normalizeStatus как есть — это и есть задуманное поведение. */
const UI_STATUS = { open: 'ready', in_progress: 'running', closed: 'done' }

export const toUiStatus = (name) => UI_STATUS[name] ?? name

export const trackerState = reactive({
  ready: false,
  generation: 0,
  columns: [],
  issues: new Map(),
  inflight: new Set(),
  health: { state: 'ok' },
  lastError: null
})

export const issueById = (id) => trackerState.issues.get(id)

/* Родство в bd выражено зависимостью parent-child, и она попадает в
   dependency_count. Считать блокировки по счётчикам нельзя — у каждой
   дочерней задачи появилось бы ложное "заблокировано 1". Считаем по
   рёбрам с типом blocks; bd отдаёт только исходящие, поэтому обратную
   сторону собираем сами. */
const dependencyCounts = computed(() => {
  const blockedBy = new Map()
  const blocks = new Map()
  for (const issue of trackerState.issues.values()) {
    const edges = (issue.dependencies ?? []).filter((d) => d.type === 'blocks')
    if (edges.length) blockedBy.set(issue.id, edges.length)
    for (const edge of edges) {
      blocks.set(edge.depends_on_id, (blocks.get(edge.depends_on_id) ?? 0) + 1)
    }
  }
  return { blockedBy, blocks }
})

export const boardColumns = computed(() => {
  const { blockedBy, blocks } = dependencyCounts.value
  const buckets = new Map(trackerState.columns.map((c) => [c.name, []]))

  for (const issue of trackerState.issues.values()) {
    // Статус, которого нет в наборе bd, всё равно должен быть виден.
    if (!buckets.has(issue.status)) buckets.set(issue.status, [])
    buckets.get(issue.status).push({
      id: issue.id,
      title: issue.title,
      status: toUiStatus(issue.status),
      blockedBy: blockedBy.get(issue.id) ?? 0,
      blocks: blocks.get(issue.id) ?? 0,
      spawnedFrom: issue.parent ?? undefined,
      state: trackerState.inflight.has(issue.id) ? 'changed' : 'default'
    })
  }

  return [...buckets].map(([name, tasks]) => ({ status: toUiStatus(name), tasks }))
})

function applyDelta(delta) {
  if (delta.columns) trackerState.columns = delta.columns
  for (const issue of delta.upserted) trackerState.issues.set(issue.id, issue)
  for (const id of delta.removed) trackerState.issues.delete(id)
  trackerState.generation = delta.generation
}

export async function resync() {
  const snapshot = await invoke('tracker_resync')
  trackerState.columns = snapshot.columns
  trackerState.issues.clear()
  for (const issue of snapshot.issues) trackerState.issues.set(issue.id, issue)
  trackerState.generation = snapshot.generation
  trackerState.ready = true
}

export async function initTracker() {
  await listen('tracker:health', (event) => {
    trackerState.health = event.payload
  })
  /* Поколение растёт на единицу с каждой дельтой. Разрыв означает, что
     событие потеряно — берём снимок целиком. */
  await listen('tracker:delta', (event) => {
    const delta = event.payload
    if (trackerState.ready && delta.generation > trackerState.generation + 1) {
      resync()
      return
    }
    applyDelta(delta)
  })

  const snapshot = await invoke('tracker_snapshot')
  trackerState.columns = snapshot.columns
  for (const issue of snapshot.issues) trackerState.issues.set(issue.id, issue)
  trackerState.generation = snapshot.generation
  trackerState.ready = true
}
```

- [ ] **Шаг 2: Написать мок для браузера**

Создать `src/stores/mockBackend.js`:

```js
/* В браузере бэкенда нет, а проверять компоненты нужно (npm run dev,
   ?view=gallery). Ставим официальный mockIPC, чтобы компоненты знали
   только invoke и listen и нигде не ветвились. */
import { mockIPC } from '@tauri-apps/api/mocks'
import { columns as fixtureColumns } from '../views/desktopAppData.js'

/* Обратный перевод: фикстуры написаны в терминах дизайн-системы,
   а бэкенд отдаёт статусы bd. */
const BD_STATUS = { ready: 'open', running: 'in_progress', done: 'closed' }

const COLUMN_CATEGORY = {
  open: 'active',
  in_progress: 'wip',
  blocked: 'wip',
  'needs-you': 'wip',
  'awaiting-review': 'wip',
  closed: 'done'
}

function fixtureIssues() {
  return fixtureColumns.flatMap((column) =>
    column.tasks.map((task) => ({
      id: task.id,
      title: task.title,
      status: BD_STATUS[task.status] ?? task.status,
      updated_at: '2026-07-31T00:00:00Z',
      priority: 2,
      issue_type: 'task',
      assignee: null,
      parent: task.spawnedFrom ?? null,
      labels: [],
      dependencies: Array.from({ length: task.blockedBy ?? 0 }, (_, n) => ({
        issue_id: task.id,
        depends_on_id: `${task.id}-dep-${n}`,
        type: 'blocks'
      }))
    }))
  )
}

export function installMockBackend() {
  if (window.__TAURI_INTERNALS__) return false

  const issues = fixtureIssues()
  const columns = fixtureColumns.map((c) => {
    const name = BD_STATUS[c.status] ?? c.status
    return { name, category: COLUMN_CATEGORY[name] ?? 'wip' }
  })
  const snapshot = { generation: 1, columns, issues }

  mockIPC((command) => {
    if (command === 'tracker_snapshot' || command === 'tracker_resync') return snapshot
    // Записи в браузерном режиме нет: возвращаем задачу как есть.
    return issues[0]
  }, { shouldMockEvents: true })

  return true
}
```

- [ ] **Шаг 3: Ставить мок до монтирования приложения**

Заменить `src/main.js` целиком:

```js
import { createApp } from 'vue'
import App from './App.vue'
import { installMockBackend } from './stores/mockBackend.js'
import './styles/styles.css'

// В браузере подменяем IPC фикстурами; под Tauri ничего не делает.
installMockBackend()

createApp(App).mount('#app')
```

- [ ] **Шаг 4: Подключить доску к трекеру**

В `src/views/DesktopApp.vue` заменить импорт фикстур — `columns` больше не берётся из файла:

```js
import { onMounted } from 'vue'
import { boardColumns, initTracker } from '../stores/tracker.js'
import {
  agents,
  expanded as initialExpanded,
  inspector,
  logLines,
  scope,
  tabs,
  tree
} from './desktopAppData.js'
```

Добавить после объявления `selectedTask`:

```js
onMounted(initTracker)
```

И в шаблоне заменить привязку доски:

```html
<KanbanBoard :columns="boardColumns" :selected-id="selectedTask" @select="selectedTask = $event" />
```

- [ ] **Шаг 5: Проверить браузерный режим**

```bash
npm run dev
```

Открыть по очереди и убедиться, что вид не изменился по сравнению с текущим:

- `http://localhost:5173/?theme=dark&density=comfortable`
- `http://localhost:5173/?theme=dark&density=compact`
- `http://localhost:5173/?theme=light&density=comfortable`
- `http://localhost:5173/?theme=light&density=compact`
- `http://localhost:5173/?view=gallery`

Доска должна выглядеть как раньше: те же пять колонок, те же карточки. Ошибок в консоли быть не
должно.

- [ ] **Шаг 6: Проверить живое обновление в приложении**

```bash
npm run tauri dev
```

Ожидается: доска показывает реальные задачи из `bd list`, а не фикстуры. Затем в терминале:

```bash
bd create "проверка доски" -t task -p 2
bd update <id-созданной> -s in_progress
```

Карточка должна появиться и переехать в колонку `in_progress` сама, без действий в интерфейсе,
примерно за секунду.

- [ ] **Шаг 7: Коммит**

```bash
git add -A
git commit -m "feat: доска показывает реальные задачи из bd"
```

---

### Задача 9: создание, правка и закрытие задач

**Файлы:**
- Создать: `src/components/kanban/NewTaskModal.vue`
- Изменить: `src/stores/tracker.js`, `src/views/DesktopApp.vue`, `src/components/index.js`,
  `src/views/Gallery.vue`

**Интерфейсы:**
- Потребляет: `trackerState`, `issueById` из задачи 8
- Отдаёт: `createIssue(newIssue)`, `updateIssue(id, patch)`, `closeIssue(id, reason)`,
  `reopenIssue(id)` из `tracker.js`; компонент `NewTaskModal`.

- [ ] **Шаг 1: Добавить операции записи в хранилище**

Дописать в конец `src/stores/tracker.js`:

```js
/* Запись занимает около двух секунд, поэтому изменение применяется сразу,
   а элемент помечается как "в полёте". Пометка идёт через state карточки и
   data-attention — не цветом: цвет в этой системе принадлежит статусу. */
async function write(id, optimistic, run) {
  const before = id ? trackerState.issues.get(id) : null
  if (before && optimistic) trackerState.issues.set(id, { ...before, ...optimistic })
  if (id) trackerState.inflight.add(id)
  trackerState.lastError = null

  try {
    const issue = await run()
    trackerState.issues.set(issue.id, issue)
    return issue
  } catch (error) {
    if (before) trackerState.issues.set(id, before)
    trackerState.lastError = String(error)
    throw error
  } finally {
    if (id) trackerState.inflight.delete(id)
  }
}

export function createIssue(issue) {
  return write(null, null, () => invoke('tracker_create', { issue }))
}

export function updateIssue(id, patch) {
  const optimistic = {}
  if (patch.title !== undefined) optimistic.title = patch.title
  if (patch.status !== undefined) optimistic.status = patch.status
  if (patch.priority !== undefined) optimistic.priority = patch.priority
  if (patch.assignee !== undefined) optimistic.assignee = patch.assignee
  return write(id, optimistic, () => invoke('tracker_update', { id, patch }))
}

export function closeIssue(id, reason = null) {
  return write(id, { status: 'closed' }, () => invoke('tracker_close', { id, reason }))
}

export function reopenIssue(id) {
  return write(id, { status: 'open' }, () => invoke('tracker_reopen', { id }))
}
```

- [ ] **Шаг 2: Написать форму создания**

Создать `src/components/kanban/NewTaskModal.vue`:

```vue
<script setup>
import { computed, ref } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import Select from '../core/Select.vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  busy: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'submit'])

// Типы и приоритеты — те, что понимает bd.
const TYPES = ['task', 'bug', 'feature', 'chore', 'epic', 'decision']
const PRIORITIES = [
  { value: '0', label: 'P0 · самый высокий' },
  { value: '1', label: 'P1' },
  { value: '2', label: 'P2' },
  { value: '3', label: 'P3' },
  { value: '4', label: 'P4 · самый низкий' }
]

const title = ref('')
const issueType = ref('task')
const priority = ref('2')
const description = ref('')

const valid = computed(() => title.value.trim().length > 0)

const submit = () => {
  if (!valid.value || props.busy) return
  emit('submit', {
    title: title.value.trim(),
    issue_type: issueType.value,
    priority: Number(priority.value),
    description: description.value.trim() || null
  })
  title.value = ''
  description.value = ''
}

const fields = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }
const row = { display: 'flex', gap: 'var(--space-4)' }
const label = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)',
  marginBottom: 'var(--space-3)'
}
const field = { flex: 1, minWidth: 0 }
</script>

<template>
  <Modal :open="open" title="New task" description="Goes straight into the tracker." @close="$emit('close')">
    <div :style="fields">
      <div>
        <div :style="label">Title</div>
        <Input v-model="title" placeholder="What needs doing" />
      </div>
      <div :style="row">
        <div :style="field">
          <div :style="label">Type</div>
          <Select v-model="issueType" :options="TYPES" />
        </div>
        <div :style="field">
          <div :style="label">Priority</div>
          <Select v-model="priority" :options="PRIORITIES" />
        </div>
      </div>
      <div>
        <div :style="label">Description</div>
        <Input v-model="description" placeholder="Optional" />
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!valid || busy" @click="submit">
        {{ busy ? 'Creating…' : 'Create' }}
      </Button>
    </template>
  </Modal>
</template>
```

- [ ] **Шаг 3: Опубликовать компонент**

В `src/components/index.js` добавить в блок экспортов kanban:

```js
export { default as NewTaskModal } from './kanban/NewTaskModal.vue'
```

В `src/views/Gallery.vue` добавить в секцию kanban, рядом с существующими примерами:

```html
<NewTaskModal :open="true" @close="() => {}" @submit="() => {}" />
```

а к блоку импортов в начале `Gallery.vue` — строку:

```js
import NewTaskModal from '../components/kanban/NewTaskModal.vue'
```

- [ ] **Шаг 4: Подключить создание и закрытие к экрану**

В `src/views/DesktopApp.vue` дописать в `<script setup>`:

```js
import Input from '../components/core/Input.vue'
import Select from '../components/core/Select.vue'
import NewTaskModal from '../components/kanban/NewTaskModal.vue'
import Toast from '../components/overlays/Toast.vue'
import {
  boardColumns,
  closeIssue,
  createIssue,
  initTracker,
  issueById,
  reopenIssue,
  toUiStatus,
  trackerState,
  updateIssue
} from '../stores/tracker.js'

const newTaskOpen = ref(false)
const creating = ref(false)

const selectedIssue = computed(() => (selectedTask.value ? issueById(selectedTask.value) : null))

const submitNewTask = async (issue) => {
  creating.value = true
  try {
    const created = await createIssue(issue)
    newTaskOpen.value = false
    selectedTask.value = created.id
  } catch {
    // сообщение уже лежит в trackerState.lastError
  } finally {
    creating.value = false
  }
}

const renameSelected = (title) => updateIssue(selectedTask.value, { title }).catch(() => {})
const setSelectedStatus = (status) => updateIssue(selectedTask.value, { status }).catch(() => {})
const closeSelected = () => closeIssue(selectedTask.value).catch(() => {})
const reopenSelected = () => reopenIssue(selectedTask.value).catch(() => {})

const statusOptions = computed(() => trackerState.columns.map((c) => c.name))
```

В шаблоне: в правой колонке, когда карточка выбрана, показывать её поля вместо фикстурного блока.
Заменить содержимое `inspectorBody` на условное — фикстурный блок остаётся веткой «ничего не
выбрано», чтобы задуманный громкий момент экрана никуда не делся:

```html
<div :style="inspectorBody">
  <template v-if="selectedIssue">
    <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }">
      <span :style="{ font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }">
        {{ selectedIssue.id }}
      </span>
      <StatusBadge :status="toUiStatus(selectedIssue.status)" size="sm" />
    </div>
    <Input :model-value="selectedIssue.title" @update:model-value="renameSelected" />
    <Select :model-value="selectedIssue.status" :options="statusOptions" @update:model-value="setSelectedStatus" />
    <div :style="{ display: 'flex', gap: 'var(--space-4)' }">
      <Button v-if="selectedIssue.status !== 'closed'" variant="secondary" size="sm" @click="closeSelected">
        Close
      </Button>
      <Button v-else variant="secondary" size="sm" @click="reopenSelected">Reopen</Button>
    </div>
  </template>

  <template v-else>
    <!-- прежний фикстурный блок целиком, без изменений -->
  </template>

  <LogView ... />
</div>
```

Кнопку создания добавить в микрозаголовок центральной колонки, над доской:

```html
<div :style="{ display: 'flex', justifyContent: 'flex-end', padding: '0 var(--panel-pad)' }">
  <Button variant="primary" size="sm" icon="plus" @click="newTaskOpen = true">New task</Button>
</div>
<NewTaskModal :open="newTaskOpen" :busy="creating" @close="newTaskOpen = false" @submit="submitNewTask" />
```

Иконка `plus` в `src/components/core/icons.js` уже зарегистрирована — регистрировать ничего не нужно.

- [ ] **Шаг 5: Показывать ошибку записи**

В конец шаблона `DesktopApp.vue`, внутри корневого `div`:

```html
<div v-if="trackerState.lastError" :style="{ position: 'fixed', right: 'var(--space-6)', bottom: 'var(--space-6)', zIndex: 10 }">
  <Toast tone="error" title="Не удалось записать в трекер" :description="trackerState.lastError"
         @close="trackerState.lastError = null" />
</div>
```

Тон `error` — один из четырёх, которые понимает `Toast` (`info`, `success`, `warning`, `error`).

- [ ] **Шаг 6: Проверить браузерный режим**

```bash
npm run dev
```

Все четыре сочетания темы и плотности плюс `?view=gallery`. Ожидается: экран цел, форма создания
открывается и закрывается, в галерее появился новый компонент. Записи в браузере нет — это
ожидаемо, мок возвращает заглушку.

- [ ] **Шаг 7: Проверить запись вживую**

```bash
npm run tauri dev
```

Пройти сценарий целиком:

1. Нажать «New task», завести задачу — она появляется на доске, примерно через две секунды карточка
   перестаёт быть «в полёте».
2. Проверить в терминале: `bd list --json` содержит новую задачу.
3. Выбрать карточку, сменить статус в инспекторе — карточка переезжает в другую колонку.
4. Нажать «Close» — карточка уходит в колонку завершённых, `bd show <id>` показывает `closed`.
5. Нажать «Reopen» — возвращается.

- [ ] **Шаг 8: Коммит**

```bash
git add -A
git commit -m "feat: создание, правка и закрытие задач из интерфейса"
```

---

## Самопроверка плана

**Покрытие спецификации.** Разделы 4 и 5 спеки (модули и контракт) — задачи 3, 4, 5, 7; раздел 6
(модель и словарь статусов) — задачи 3 и 8; раздел 7 (движок синхронизации) — задачи 6 и 7;
раздел 8 (поставка bd) — задача 2; раздел 9 (оптимистичность и ошибки) — задача 9; раздел 10
(браузерный режим) — задача 8; раздел 11 (проверка) — шаги проверки в каждой задаче.

**Осознанно не покрыто планом.** Внедрение вшитого bd в `PATH` порождаемых агентов (раздел 8 спеки)
не реализуется: приложение пока не запускает агентов, реализовывать нечего. Это остаётся за планом
и всплывёт вместе с работой над агентами.

**Известная хрупкость.** `since_with_overlap` использует `chrono` для вычитания пяти секунд;
если формат `updated_at` в будущей версии bd изменится, разбор молча вернёт эпоху и каждый тик
превратится в полную выборку. Поведение останется корректным, но перестанет быть дешёвым.
