<h1 align="center">
  <img src="assets/icon.png" alt="" height="64" align="absmiddle">&nbsp;Smetana
</h1>

<p align="center">
  <a href="https://github.com/invisor/smetana/releases/latest"><img alt="最新版本" src="https://img.shields.io/github/v/release/invisor/smetana"></a>
  <img alt="macOS Apple silicon" src="https://img.shields.io/badge/macOS-Apple%20silicon-111?logo=apple&logoColor=white">
  <a href="https://claude.com/claude-code"><img alt="支持 Claude Code" src="https://img.shields.io/badge/agent-Claude%20Code-d97757"></a>
  <a href="https://github.com/openai/codex"><img alt="支持 Codex" src="https://img.shields.io/badge/agent-Codex-10a37f"></a>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README.ru.md">Русский</a> ·
  <b>中文</b>
</p>

<p align="center">
  <b>自主编码。建好任务，启动 run，去喝杯咖啡。</b><br>
  把你想要的说清楚，智能体会把它拆成任务，问清自己定不了的地方，并在任务之间建好阻塞与依赖关系。你只按一次 play，接下来 run 会自己走完整个队列。剩下的事只是看结果。
</p>

<h3 align="center"><a href="https://github.com/invisor/smetana/releases/latest">下载 Smetana</a></h3>

![看板：左边是项目的智能体会话，右边是选中的任务](assets/screenshot-board.png)

## 这是什么

Smetana 是一个 ADE —— agentic development environment，围绕智能体构建的开发环境。它不是一个加了聊天框的编辑器，而是这样一个地方：你说明产品该做什么，活由智能体来干。应用内置了两样东西。一是看板追踪器 [bd](https://github.com/gastownhall/beads)，它把任务放在你自己的仓库里，而不是别人的服务器上。二是技能库 [Superpowers](https://github.com/obra/superpowers)，智能体的做事方法来自这里 —— 如何把一个任务问清楚、如何规划、如何评审改动、如何合并。

### 一件事情是怎么走完的

1. **你用自己的话描述它。** 一段话、一张截图都行，不需要规范的工单格式。
2. **智能体来提问。** 它和你一起过一遍所有含糊的地方，直到没有要紧的东西需要靠猜。
3. **智能体建任务** —— 一个 issue，或者多个并在它们之间连好依赖 —— 然后放进 Ready。
4. **你启动一次 run**，并指定结果要落到哪个 git 分支。
5. **run 从 Ready 取一批任务**，在允许的地方并行推进，每个任务都在自己的 worktree 里，两个任务不会踩在同一个工作副本上。
6. **把通过的合并**进那个分支，包括处理冲突。
7. **再取下一批**，如此继续，直到 Ready 空了。
8. **你把项目跑起来，看看做出了什么。**

在诚实的前提下，应用尽量让人不必参与其中。建任务、改任务、解决合并冲突、判断一个分支能不能合 —— 这些是智能体自己的活；只有在没有你就定不下来的地方才会找你：要么是写进任务备注里的一个问题，要么是一次停下来并指明卡在哪个任务上的 run。它知道的一切都放在你能打开的文件里 —— 任务在 `.beads/`，run 的状态和报告在 `.smetana/`。没有服务器，没有账号，没有数据库。

## 看板

一列就是一种任务状态，看板会画出项目里的所有状态。下面这些是工作真正围绕的：

- **Ready** —— 可以开工：问题都问清楚了，需要实现方案的地方也写好了，没有未完成的东西卡着它。run 从这里取一批任务。
- **Running** —— 正在被处理：某个智能体已经认领并在做。
- **Blocked** —— 这里有两种卡片。一种在等另一个任务：它依赖的东西还没做完，那个一关闭，它就自己回到 Ready。另一种带着锁的徽标：是人手动锁上的，只有同一个人解锁才能把它放回去。
- **Parked** —— 干活时总会冒出写任务时无法预见的情况，其中一些需要人来定。run 途中碰上这种情况的智能体会把任务 park 起来：问题写进任务备注，**Answer questions** 会启动一个智能体来向你提问，拿到答复后把任务送回 Ready。run 本身会继续 —— 当前这一批结束，下一批照样从 Ready 取，只是跳过依赖这个被 park 任务的部分。只有同一个问题第二次出现，才会让 run 结束。
- **Ready to merge** —— 做完并评审过，等着合进目标分支。合进去之后就关闭。
- **Human check** —— 已完成并合并，但还等人亲眼看一下。当 run 没法自己验证成果时会留下这样一个任务：你看过之后，要么关掉它，要么把它退回 Ready。
- **Done** —— 做完并已关闭。
- **Deferred** —— 有意推迟，并没有东西卡着它。做别的任务时顺手发现的问题会落在这里：智能体绊到一个 bug，会把它记进 Deferred 而不是 Ready —— 这是刻意的，一个会把自己发现的问题也捡起来做的 run 永远走不到队列尽头。只有人能把它挪回 Ready。
- **Pinned** —— 永远不会被拿去做。换句话说就是 backlog：将来某天要做、但现在不做的任务。
- **Hooked** —— 某个智能体一次性接走了一整组相关任务。它说明这活归谁，而不是进展到哪一步；run 不会碰这些。

项目可以有自己的列。bd 里有的任何状态都会变成一列，应用会为它挑一个颜色和一个两字母的代号。

## Run

一次 run 是应用内部推动工作的一个进程：它读看板，把一批任务交给智能体会话，等它们做完，合并通过的部分，再读一次看板。每一批都有自己的会话，所以上下文每轮都从干净的状态开始。它同时盯着订阅额度：额度用完时，run 就地暂停，等额度恢复后继续。

run 可以针对单个任务、一个 epic，或者整个 Ready 队列启动。然后你指定结果落在哪里 —— 目标分支 —— 同时推进多少个任务(1 到 8，默认 3)，以及针对队列的优先级下限，低于它的任务不会被取(默认 P2 及以上)。

**Solo** —— 一个任务，智能体自己动手，不把活分给别人。它会随时向你提问，并等到你回答再继续。只有在你指定了具体任务时才提供：对 epic 和队列来说，“单干”没有意义。这是控制感最强的模式。

**Crew** —— 一个 lead 智能体带着若干队友会话。队友的问题由 lead 来回答，只有 lead 自己定不下来的才会找你；在你思考的时候任务不会被 park —— 会话在等你。一次 Crew run 只取一批任务，合并完就结束，所以它适合贴近地跟着工作走。

**Autopilot** —— 没人在场时的编码。run 从 Ready 取一批，推到合并，再取下一批，一直到它范围内的队列空了，或者遇到需要人来定的事情。需要人来定的会被 park，问题写在任务备注里，run 则继续处理与之无关的任务。这里的会话是无人值守的：做完活就退出。

Crew 和 Solo 里，会话比 run 活得久 —— 它停在提示符上，你可以接着跟它聊，run 结束不会关掉它。Autopilot 不是这样，那个模式本来就是为没人在场的房间设计的。

**多个 run 可以同时进行**：同一个项目里针对不同范围 —— 队列和某个 epic 并行 —— 以及多个项目同时进行。只有一件事被拒绝：针对*同一个*范围的第二个 run，那会变成两个 lead 抢同一批任务。

还有两个开关。**Check each task for real before closing it** 会让智能体把项目真正跑起来验证成果，而不是只信自己的测试。**File what it finds along the way** 允许它把绊到的 bug 记下来 —— 记进 Deferred，永远不会进 Ready。

<p align="center">
  <img src="assets/screenshot-run.png" alt="启动对话框：目标分支、模式、同时推进多少个任务、优先级下限" width="420">
</p>

不管 run 以什么方式结束，它都会写一份报告：`.smetana/reports/` 下一个自包含的 HTML 文档，说明关闭了什么、park 了什么、总共花了多久，以及是哪个模式做的。

## 环境要求

- **macOS，Apple silicon（arm64）** —— 目前唯一发布的构建，也是应用唯一实际跑过的平台。Windows 和 Linux 都是想要且已计划的：Tauri 两边都能构建，发布流水线里各留好了一行，只是都还没有人亲眼验过。
- **装好并登录的 CLI harness** —— [Claude Code](https://claude.com/claude-code) 或 [Codex](https://github.com/openai/codex)。应用驱动的是你本来就有的命令行工具；它不是模型客户端，也不保存任何密钥。两者都支持，但到目前为止几乎所有测试都在 Claude Code 上。
- **git。**

## 安装

从 [Releases](https://github.com/invisor/smetana/releases) 下载 `.dmg`，把 Smetana 拖进 Applications。它用 Apple Developer ID 签名并做过公证，双击即可打开：没有需要关掉的提示，也不需要事先授权。

## 上手

1. **添加项目。** 点击左侧项目栏的 `+` 并选择文件夹。受版本控制的仓库里的任意子目录会解析到仓库根目录；如果里面还没有 bd 追踪器，应用会主动提出替你执行 `bd init`。
2. **看板会加载**该仓库的 `.beads/`，并持续跟随它的变化 —— 无论是谁改的：这个窗口、某个智能体，还是你在终端里。
3. **为 run 配置项目。** 项目磁贴的菜单里有 **Set up**：它会启动一个智能体会话，向你了解项目并写出 `.smetana/project.toml` —— 项目由哪些仓库组成、工作合到哪个分支、用什么命令把它跑起来、任务合并前要过哪些关卡。`.smetana/` 会被加进 `.gitignore`，所以这些都不会被提交。
4. **建任务**：点击某一列顶部的 `+`，用自己的话写清楚要什么；其余的智能体会问你。
5. **按下 play** —— 在卡片上、epic 上或队列上 —— 然后选择模式、目标分支，以及同时推进多少个任务。

## 参与进来

应用还很早期，而且是公开开发的，因为只有这样才能找出粗糙的地方。**出问题了？[提一个 issue](https://github.com/invisor/smetana/issues)** —— 说明出问题时你正想做什么。**有想法、有期望，或者想问这东西该怎么用？[开一个讨论](https://github.com/invisor/smetana/discussions)。** 路线图是刻意保持简短的，大家真正提出的需求会往上排。

**也欢迎帮忙写代码。** 眼下最有帮助的是这四件事：

- **验证 Windows 和 Linux 的构建。** 发布流水线里为两个平台各留了一行；还没有人在上面跑过这个应用。
- **在 Codex 上跑一整夜。** 两个 harness 都支持，但到目前为止几乎所有测试都在 Claude Code 上。
- **为其他 harness 写智能体 profile。** 应用要求智能体做的每件事都只写一次，再按 harness 翻译，所以加第三个是写一份 profile，而不是重写。
- **任何在你那儿坏掉的东西** —— 如果牵涉到 run，请附上 `.smetana/reports/` 里的运行报告：它记录了应用当时认为正在发生什么。

改动 `src/` 下的任何东西之前，请先读 [`CLAUDE.md`](CLAUDE.md)：前端是一套设计系统的移植，它的规则不能逐个组件地商量。[`AGENTS.md`](AGENTS.md) 是针对在这个仓库里工作的智能体的同类文档。

## 许可证

[MIT](LICENSE)。
