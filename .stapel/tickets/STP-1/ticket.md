# STP-1 — рабочая область cargo, `stapel init`, `stapel.toml`, хуки

## Описание

Источник: `docs/PHASES.md`, фаза 0, первый тикет. Внешнего трекера нет.

Нужна рабочая область cargo из семи пакетов и первая команда `stapel init`, которая готовит чужой
репозиторий к работе: создаёт `.stapel/` со стартовым `stapel.toml` и ставит хуки Claude Code, запрещающие
агентам `git push` и запись в код до разрешения сборки. Закрывает критерий 1 фазы 0 и часть критерия 7.

## Спека

### Критерии приёмки

Каждый критерий записан в форме «кто · что · с чем · при каком условии» и имеет тест. Имена тестов
предварительные, точные пути появятся на шаге RED.

| № | Критерий | Тест |
|---|---|---|
| AC-1 | Разработчик · собирает бинарный файл `stapel` · одной командой `cargo build --release` из корня · на чистой копии репозитория; в рабочей области ровно семь пакетов из `CLAUDE.md`, пять из них — пустые заготовки библиотек. | `cli::workspace_has_seven_members`, `cli::version_prints_package_version` |
| AC-2 | Разработчик · запускает `stapel init` · в корне git-репозитория без `.stapel/` · и получает `.stapel/stapel.toml`, `.stapel/tickets/`, `.stapel/allowlist.toml` и строку `/.stapel/index/` в `.gitignore`; команда печатает список созданного и завершается с кодом 0. | `init::creates_layout_in_empty_repo`, `init::creates_layout_at_repo_root_from_subdir`, `init::appends_to_existing_gitignore` |
| AC-3 | `stapel-core` · читает `stapel.toml`, созданный `init`, · без ошибок · и видит в нём разделы `spec, design, proof, plan, review, summary`, шесть ролей из `docs/PLAN.md` §5 и правило ключа `tickets.key` с префиксом, заданным при `init`. | `core::config::default_config_roundtrips`, `core::config::default_config_has_roles_and_sections`, `core::config::config_rejects_missing_ticket_key` |
| AC-4 | Разработчик · повторно запускает `stapel init` · в уже инициализированном репозитории · и ни один файл не меняется: содержимое и время изменения те же; команда печатает «уже готово» и завершается с кодом 0. | `init::second_run_changes_nothing` |
| AC-5 | Разработчик · запускает `stapel init` · когда `stapel.toml` уже есть и отредактирован вручную · и файл остаётся как есть; недостающие части раскладки и хуки дописываются. | `init::keeps_user_edited_config` |
| AC-6 | `stapel init` · ставит хуки в `.claude/settings.json` · когда файла нет, когда он есть с чужими ключами и хуками, и при повторном запуске · так, что чужие ключи и хуки сохранены, а записи `stapel` не дублируются. | `init::hooks_into_missing_settings`, `init::hooks_merge_with_foreign_settings`, `init::hooks_not_duplicated` |
| AC-7 | Хук `stapel hook pre-tool-use` · получает от Claude Code вызов `Bash` · с командой, которая выполняет `git push` в любом виде (`git push`, `git -C dir push`, `cd x && git push`, `FOO=1 git push`, `git push` после `;` или `\|\|`) · и отвечает отказом с причиной; команды `git status`, `git log --grep push`, `echo "git push"` пропускает. | `hook::denies_git_push_variants`, `hook::allows_non_push_commands` |
| AC-8 | Хук `stapel hook pre-tool-use` · получает вызов `Write`, `Edit`, `MultiEdit` или `NotebookEdit` · для пути вне списка `guard.always_writable` из `stapel.toml` · когда ни у одного тикета сборка не разрешена · и отвечает отказом с причиной «сборка не разрешена»; пути внутри `.stapel/` и `docs/` пропускает всегда. | `hook::denies_code_write_without_build`, `hook::allows_stapel_and_docs_writes` |
| AC-9 | Хук · получает вызов записи в код · когда в `.stapel/tickets/<ключ>/state.json` есть `"build": {"allowed": true}` · и пропускает его. | `hook::allows_code_write_when_build_allowed` |
| AC-10 | Хук · получает вход, который не может разобрать, или вызывается вне репозитория со `stapel.toml` · и не падает: неразборчивый вход отклоняется с причиной, вне репозитория вызов пропускается. | `hook::rejects_garbage_input`, `hook::passes_outside_stapel_repo` |
| AC-11 | Разработчик · запускает `stapel init` · вне git-репозитория · и получает отказ с кодом 1 и сообщением «не git-репозиторий»; ничего не создаётся. | `init::refuses_outside_git` |
| AC-12 | Разработчик · запускает первый `stapel init` · с `--prefix ABC` или отвечая на вопрос в терминале · и получает `tickets.key = "ABC-{n}"` в `stapel.toml`; префикс — от 2 до 8 заглавных латинских букв, иначе отказ с причиной; без терминала и без `--prefix` — отказ с кодом 1 и подсказкой `--prefix`, ничего не создаётся; повторный `init` префикс не спрашивает. | `init::prefix_from_flag`, `init::prefix_from_prompt`, `init::rejects_bad_prefix`, `init::refuses_without_prefix_noninteractive` |

### Вопросы

Вопросы с рекомендацией. Все пять получили ответ.

1. **Идентификатор модели для сборщика и ревьюеров.** В `docs/PLAN.md` §5 записано `claude-sonnet-5`, а
   текущая модель семейства — `claude-sonnet-5-5`. Рекомендация: в стартовом `stapel.toml` писать
   `claude-sonnet-5-5` и поправить `PLAN.md` тем же коммитом. Ответ (2026-10-02, человек): поправить.
   `PLAN.md` исправлен коммитом этого тикета.
2. **Как хук узнаёт, что сборка разрешена.** `state.json` появляется только в STP-2. Рекомендация: STP-1
   фиксирует минимальный контракт — поле `build.allowed` в `state.json` любого тикета, — а STP-2 начинает
   его писать. Альтернатива — отложить запрет записи до STP-2. Ответ (2026-10-02, человек): по рекомендации.
3. **Что делает хук, если бинарного файла `stapel` нет в `PATH`.** Claude Code считает ошибку команды хука
   (код возврата не 2) незапрещающей, то есть защита тихо пропадает. Рекомендация: `init` проверяет `PATH` и
   предупреждает; в `settings.json` пишется `stapel hook pre-tool-use` без абсолютного пути, чтобы файл
   оставался переносимым между машинами. Ответ (2026-10-02, человек): по рекомендации.
4. **Ставить ли `init` в этот репозиторий в конце STP-1.** Тогда запрет записи сразу заработает для
   оркестратора, а `state.json` в фазе 0 ведётся руками. Рекомендация: да, это первый настоящий прогон на
   чужом репозитории; `state.json` для STP-2 оркестратор создаёт руками с `build.allowed = true` после
   подтверждений. Ответ (2026-10-02, человек): по рекомендации.
5. **Префикс ключа в чужом репозитории.** Рекомендация: `init --prefix <ABC>`, по умолчанию `STP`. Ответ
   (2026-10-02, человек): ключ хранится в конфиге, `init` спрашивает его, если он не задан. Итог: правило
   ключа — поле `tickets.key` в `stapel.toml`; `init` берёт префикс из `--prefix`, иначе спрашивает в
   терминале; без терминала и без `--prefix` — отказ с подсказкой. Умолчания `STP` нет. Критерий AC-12.

### Не входит

- Команды `new`, `ok`, `status`, создание и изменение `state.json` (STP-2).
- Журналы решений и токенов как команды (STP-3); журнал этого тикета ведётся руками.
- Запрет записи через `Bash` (`echo > file`, `sed -i`, `cargo fmt`): хук смотрит только на инструменты
  записи. Это известная дыра, закрывается ревью диффа; отдельное решение — в фазе 1.
- Хук «ревьюер без записи в git»: ревьюеры фазы 0 работают на `git archive` без `.git`, хук не нужен.
- Хуки для других агентов, кроме Claude Code.
- Удаление хуков (`stapel deinit`).

## Решение

**Пакеты.** `stapel-core` — библиотека: тип `Config` (serde, `toml`), стартовый `stapel.toml` как
константа, чистая функция решения хука `decide(input, repo) -> Decision`. `stapel-cli` — бинарный файл:
`clap`, подкоманды `init` и `hook pre-tool-use`, ввод-вывод. Остальные пять — `lib.rs` с комментарием о
назначении.

**`init`.** Корень — `git rev-parse --show-toplevel`. Каждый файл пишется, только если его нет или его
содержимое должно измениться; это даёт AC-4 без отдельной логики. Префикс ключа спрашивается, только
когда `stapel.toml` ещё нет и stdin — терминал (`std::io::IsTerminal`); тест AC-12 подаёт ответ через stdin с
переменной `STAPEL_ASSUME_TTY=1`, которая нужна только для тестов и описана в `--help` как служебная. `settings.json` читается как
`serde_json::Value`, запись `stapel` опознаётся по строке команды и добавляется, только если её нет.

**Хук.** Читает JSON Claude Code из stdin (`tool_name`, `tool_input`, `cwd`). Отказ — код 2 и причина в
stderr: это задокументированный способ запрета, его видит и агент, и человек. Разбор команды `Bash` — по
токенам `shell-words` после разбиения на `&&`, `||`, `;`, `|`; перед `git` пропускаются присваивания
переменных, у `git` — глобальные флаги (`-C`, `-c`, `--git-dir` и подобные), затем проверяется подкоманда.

**Варианты, которые отброшены.** Регулярное выражение по всей строке — ложные срабатывания на
`echo "git push"` и `git log --grep push`. Абсолютный путь к бинарному файлу в `settings.json` — ломает
переносимость отслеживаемого файла.

**Риски.**

| Риск | Тест |
|---|---|
| R-1 Разбор команды пропустит вариант `git push` | `hook::denies_git_push_variants` (таблица из AC-7, дополняется по находкам) |
| R-2 `init` испортит существующий `settings.json` | `init::hooks_merge_with_foreign_settings` (сравнение чужих ключей до и после) |
| R-3 Повторный `init` перезапишет файлы с тем же содержимым и сменит время изменения | `init::second_run_changes_nothing` |

## Доказательство

Результат подставляет инструмент; в фазе 0 — оркестратор по выводу `cargo test`.

| Критерий или риск | Тест | Результат |
|---|---|---|
| AC-1 | `cli::workspace_has_seven_members`, `cli::version_prints_package_version` | — |
| AC-2 | `init::creates_layout_in_empty_repo`, `init::creates_layout_at_repo_root_from_subdir`, `init::appends_to_existing_gitignore` | — |
| AC-3 | `core::config::default_config_roundtrips`, `core::config::default_config_has_roles_and_sections`, `core::config::config_rejects_missing_ticket_key` | — |
| AC-4, R-3 | `init::second_run_changes_nothing` | — |
| AC-5 | `init::keeps_user_edited_config` | — |
| AC-6, R-2 | `init::hooks_into_missing_settings`, `init::hooks_merge_with_foreign_settings`, `init::hooks_not_duplicated` | — |
| AC-7, R-1 | `hook::denies_git_push_variants`, `hook::allows_non_push_commands` | — |
| AC-8 | `hook::denies_code_write_without_build`, `hook::allows_stapel_and_docs_writes` | — |
| AC-9 | `hook::allows_code_write_when_build_allowed` | — |
| AC-10 | `hook::rejects_garbage_input`, `hook::passes_outside_stapel_repo` | — |
| AC-11 | `init::refuses_outside_git` | — |
| AC-12 | `init::prefix_from_flag`, `init::prefix_from_prompt`, `init::rejects_bad_prefix`, `init::refuses_without_prefix_noninteractive` | — |

## План

Каждый шаг — пара коммитов RED (только тесты), затем GREEN (код). Команда проверки на всех шагах —
`cargo test --workspace`; на RED ожидается падение названных тестов (или ошибка компиляции только в них),
на GREEN — все тесты зелёные.

| Шаг | Что | Тесты | Ожидание на RED |
|---|---|---|---|
| 1 | Рабочая область, семь пакетов, `stapel --version` | AC-1 | бинарного файла нет |
| 2 | `Config` и стартовый `stapel.toml` в `stapel-core` | AC-3 | типа `Config` нет |
| 3 | `init`: раскладка, `.gitignore`, отказ вне git, префикс ключа | AC-2, AC-11, AC-12 | подкоманды нет |
| 4 | `init`: повторный запуск и ручные правки | AC-4, AC-5 | файлы перезаписываются |
| 5 | `init`: хуки в `.claude/settings.json` | AC-6 | хуков нет |
| 6 | Хук: запрет `git push` | AC-7, AC-10 | подкоманды `hook` нет |
| 7 | Хук: запрет записи до разрешения сборки | AC-8, AC-9 | запись пропускается |
| 8 | `init --prefix STP` на этом репозитории | ручная проверка, вывод в журнал | — |

Зависимости: `clap`, `serde`, `serde_json`, `toml`, `shell-words`, `anyhow`; для тестов `assert_cmd`,
`tempfile`.

## Ревью

Генерируется. В фазе 0 — три ревьюера на `git archive` итогового коммита: свежий ревьюер кода, внешний
ревьюер, ревьюер расхождений.

## Итог

Генерируется после слияния.
