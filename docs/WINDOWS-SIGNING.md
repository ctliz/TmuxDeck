# Windows 代码签名配置指南（Azure Trusted Signing）

> TmuxDeck 的 Release workflow 已支持 Azure Trusted Signing。
> 本指南带你完成**一次性配置**，之后每次发版 Windows 安装包自动签名，
> 用户下载时不会再看到 SmartScreen「Windows 已保护你的电脑」红色警告。
>
> ⏱ 全程约 30 分钟（不含审核等待）。**只需做一次。**

---

## 原理

微软提供 **Azure Trusted Signing** 代码签名服务（按量计费，无免费层）。GitHub Actions 官方 action
直接支持，签名后 Windows 安装包显示「发布者已验证」。

**为什么 macOS 不做？** macOS 公证强制要求 $99/年的 Apple Developer 账号，
纯开源项目不值得。Windows 的 Trusted Signing 虽然按量计费，但对低频发版项目
每次发版仅 2 次签名（msi + exe），月度成本约几分钱（前提是绑定信用卡）。

---

## 前置条件

- 一个 **Azure 账号**（没有就注册 https://azure.microsoft.com/free ，免费订阅即可）
- GitHub 仓库 `ctliz/TmuxDeck`（已是开源仓库）

---

## 第 1 步：创建 Trusted Signing 资源

1. 打开 https://portal.azure.com 并登录
2. 顶部搜索框输入 **Trusted Signing** → 点击 **Trusted Signing accounts**
3. 点 **+ Create**
4. 填写：
   - **Subscription**：选你的订阅（免费订阅即可）
   - **Resource group**：新建一个，比如 `tmuxdeck-rg`
   - **Account name**：比如 `tmuxdecksigning`（记下来）
   - **Region**：选就近的（如 East US / East Asia）
5. **Review + create** → Create
6. 等待部署完成，进入该资源

## 第 2 步：创建证书 Profile

1. 在 Trusted Signing 资源页左侧选 **Certificate profiles** → **+ Create**
2. 填写：
   - **Profile name**：比如 `tmuxdeck-profile`（记下来）
   - **Profile type**：选 **Basic Validation**（公信任，用户能直接看到发布者）
3. 创建后，profile 会进入 **验证中（pending validation）** 状态
   - Microsoft 会审核你的身份，**通常 1-3 个工作日**
   - 审核通过后状态变 Active，此时才能签名
4. 在 profile 详情页右上角查看 **Endpoint**（形如 `https://xxx.trustedsigning.azure.net`，记下来）

> 审核期间可以先跳过签名正常发版（workflow 会自动跳过签名步骤），
> 等审核过了、secrets 配好，下一版自动生效。

## 第 3 步：创建 GitHub Actions 可用的身份（OIDC）

GitHub Actions 要用你的 Azure 身份签名，需要一个「服务主体」+ 联邦凭证：

1. 打开 https://portal.azure.com → 搜索 **Microsoft Entra ID**（原 Azure AD）
2. 左侧 **App registrations** → **+ New registration**
   - **Name**：`tmuxdeck-oidc`
   - 其余默认 → **Register**
3. 记下 **Application (client) ID** 和 **Directory (tenant) ID**
4. 左侧 **Certificates & secrets** → **Federated credentials** → **+ Add credential**
   - **Federated credential scenario**：选 **GitHub Actions deploying Azure resources**
   - **Organization**：`ctliz`
   - **Repository**：`TmuxDeck`
   - **Entity type**：**Environment**（选一个 environment，如 `release`）
     > 用 Environment 比 Branch 安全，且不随分支名变化。
   - **Name**：`tmuxdeck-release`
   - **Subject identifier** 会自动生成，确认格式为 `repo:ctliz/TmuxDeck:environment:release`
   - **Add**
5. 记下你的 **Subscription ID**：
   - 搜索 **Subscriptions** → 点击你的订阅 → 复制 **Subscription ID**

> 提示：Environment 需要在 GitHub 仓库 Settings → Environments 里手动创建名为 `release` 的 environment（不需要配置任何保护规则）。

## 第 4 步：把配置写入 GitHub Secrets

打开 https://github.com/ctliz/TmuxDeck/settings/secrets/actions ，新增：

| Secret 名 | 值 |
|---|---|
| `AZURE_CLIENT_ID` | 第 3 步的 Application (client) ID |
| `AZURE_TENANT_ID` | 第 3 步的 Directory (tenant) ID |
| `AZURE_SUBSCRIPTION_ID` | 第 3 步的 Subscription ID |
| `AZURE_TRUSTED_SIGNING_ENDPOINT` | 第 2 步的 Endpoint（如 `https://xxx.trustedsigning.azure.net`） |
| `AZURE_TRUSTED_SIGNING_ACCOUNT` | 第 2 步的 Account name |
| `AZURE_TRUSTED_SIGNING_CERT_PROFILE` | 第 2 步的 Profile name |

> 这些全部存为 **Repository secrets**（仓库级），Actions 和 Environments 都能用。
> workflow 已经写好：**这 6 个 secret 任意一个为空 → 自动跳过签名步骤**，不会让发版卡住。

## 第 5 步：验证

1. 打 tag 触发 Release workflow：
   ```bash
   git tag v1.2.0 && git push origin v1.2.0
   ```
2. 打开 https://github.com/ctliz/TmuxDeck/actions 看 `Release` workflow
   - `build-windows` job 中应出现 **Azure login** 和 **Sign artifacts** 两个步骤且为绿色
3. 等 draft release 生成后，下载 Windows `.exe`，右键 → 属性 → 数字签名：
   - 应显示「Microsoft 已为该文件签名」或正常签名信息，**不再有红色警告**

---

## 常见问题

**Q: 审核期间能发版吗？**
能。secrets 配好但 profile 还在 pending 时，签名步骤会失败。此时可以临时把
`AZURE_TRUSTED_SIGNING_*` 三个 secret 删掉，workflow 自动跳过签名。
审核通过后加回来即可。

**Q: 登录报错「所选用户帐户在租户'Microsoft Services'中不存在」？**
这是 Azure 最高频的入门坑。含义：你直接登录了门户，但**还没注册 Azure 免费账户**，
Azure 默认把你塞进它的内部租户，而你没有自己的租户。

解法（按顺序）：
1. 去 https://azure.microsoft.com/free/ 点 **Start free** 注册（**不是登录**）——
   需手机号验证 + 绑定银行卡（只验证不扣费），完成后会自动创建你的默认租户和免费订阅
2. 再打开 portal.azure.com，右上角头像 → **Switch directory** 确认选中你自己的租户
3. 搜索 **Subscriptions** 确认能看到免费订阅，之后才能创建 Trusted Signing 资源

若仍报错：换无痕窗口重新登录，清掉旧会话的租户上下文。

**Q: 签名会失败吗？常见原因？**
- Profile 还在 pending（等审核）
- Endpoint / Account / Profile 名字抄错
- OIDC 联邦凭证的 Environment 名与仓库不匹配

**Q: 这要花钱吗？**
Azure Trusted Signing **没有免费层**，按量计费（约 $9.99/千次签名）。但 TmuxDeck 每次发版
只需 2 次签名（msi + exe），按每月 10 版算也就 $0.20/月。真正的前提是：**必须绑定一张
信用卡**，且 $200 试用 credit 30 天后过期，之后按量从卡上扣。

对开源项目是否值得，取决于你愿不愿意为「消除 SmartScreen 警告」绑一张卡。
不愿意的话，保持未签名 + README 引导（右键打开）是完全可接受的开源常态。

**Q: macOS 版怎么办？**
不签名，保持现状。README 已有「无法验证开发者」引导。
