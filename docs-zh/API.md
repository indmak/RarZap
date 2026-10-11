# RarZap 软件对接 API 文档

> 面向：RarZap 客户端开发（Rust + GPUI，单机离线桌面工具）
> 云端基地址：`https://www.indmak.com`（备用：`https://indmak.654880040.workers.dev`）
> 文档版本：**v1.0**（2026-10-08）
> 产品：**RarZap**（站内 slug `rarzap`）· **商业版买断 $9** · **无试用期** · **1 台设备** · 许可与其它产品同一套签发体系（同一 kid/公钥，key 格式 `IND-…`）
> Creem 产品：`prod_NZq1928pUasZRXcgzBHl1`（$9 一次性）
> 版本策略：**开源版**（MIT，免费，GitHub 发布）与**商业版**（微软商店 + 激活码）**共用同一套许可 API**；只有商业版需要联网激活。
> 原则：许可判定放引擎层，UI 只拿状态枚举。

---

## 0. 两个版本与许可的关系

| | 开源版（Free） | 商业版（Commercial） |
| --- | --- | --- |
| 授权 | **MIT，免费** | **买断 $9**，一码一机 |
| 功能 | 单文件 RAR → ZIP | 开源版全部 + **批量引擎** |
| 分发 | GitHub Releases（`github.com/indmak/RarZap`） | 微软商店（预留），或官网购买（Creem） |
| 许可 | **无需 key、不联网** | **需要激活码**，联网激活/校验 |
| 试用 | —— | **无试用期**（不存在 trial） |

> ⚠️ **RarZap 没有试用，且服务端已硬拒绝**：该产品在服务端 `trial_disabled = 1`，任何 `/api/trials/start` 或 `/api/trials/status`（`slug=rarzap`）都会返回 `400 {"error":"trial_not_offered"}`。客户端不要提供试用入口、也不要调用 `/api/trials/*`。

---

## 1. 全量接口速查（商业版使用）

| 用途 | 方法 | 完整 URL |
| --- | --- | --- |
| **激活 / 换机绑定 / 刷新离线文件** | POST | `https://www.indmak.com/api/licenses/activate` |
| 在线校验（轻量，推荐） | POST | `https://www.indmak.com/api/licenses/verify` |
| 已购模块核验（当前无插件，预留） | POST | `https://www.indmak.com/api/licenses/entitlements` |
| 解绑（换机/卸载释放名额） | DELETE | `https://www.indmak.com/api/licenses/deactivate` |
| 离线验签公钥 | GET | `https://www.indmak.com/api/licenses/pubkey` |
| 公开产品目录 | GET | `https://www.indmak.com/api/products` |
| 购买入口（应用内 Buy 按钮用这个） | — | `https://www.indmak.com/buy/rarzap` |
| 支持/留言 | — | `https://www.indmak.com/support-center?subject=RarZap` |
| 开源版下载（不在本站） | — | `https://github.com/indmak/RarZap/releases` |

全部 HTTPS JSON，**无需登录态**，凭 License Key 授权。凭据一律放 **POST body**（不要放 query —— 会进访问日志）。
限流：激活/解绑同一出口 IP **每分钟 20 次**，超限 `429 rate_limited`（指数退避重试）。

产品常量（客户端可写死）：

```text
slug        = "rarzap"
productRef  = "3771c1ea-bae9-40ce-969f-afab9e474499"   // 服务端产品 ID（回填用，非密）
creemId     = "prod_NZq1928pUasZRXcgzBHl1"             // Creem 一次性产品（$9）
maxDevices  = 1
trial       = 无（不要调用 /api/trials/*）
platforms   = windows | macos | linux    // 开源版三平台；商业版经微软商店（Windows）
```

---

## 2. 激活（重点）

`POST https://www.indmak.com/api/licenses/activate`

购买后邮件里的激活码、微软商店购买后拿到的码，**走同一接口、无任何区别**。激活即绑定设备指纹；同一指纹重复激活**不占新名额**，只刷新离线文件（响应 `rebound: true`）。

```http
POST https://www.indmak.com/api/licenses/activate
Content-Type: application/json

{
  "licenseKey": "IND-0EEEE4-83A979-034D591A",
  "deviceFingerprint": "sha256:9f2c…",
  "deviceName": "WORKSTATION-AB12"
}
```

成功（200）：

```jsonc
{
  "ok": true,
  "rebound": false,                 // true = 该指纹此前已绑定过（重装/刷新文件），同样给新签文件
  "kind": "one_time",               // 提示字段（不参与签名）：one_time|temporary|subscription
  "trial": null,                    // RarZap 无试用 → 恒为 null
  "license": {
    "v": 1,
    "kid": "2026-09-B",
    "key": "IND-0EEEE4-83A979-034D591A",
    "fp": "sha256:9f2c…",
    "iat": 1791000000,              // 文件签发（秒）
    "exp": 1796184000,              // 离线文件失效（秒）= 签发 + 60 天，到期前联网刷新
    "max": 1,                       // ★ RarZap 设备上限 = 1
    "ps": 0,                        // 买断码：0
    "pe": 0,                        // 买断码：0（永久）
    "features": []                  // 已购插件 SKU（RarZap 当前无插件，空数组）
  },
  "sig": "base64url(Ed25519 签名)",   // 对 §3 规范串的签名，无 '=' 填充
  "seats": { "used": 1, "max": 1 }
}
```

**客户端必做**：拿到响应先按 §3 验签（公钥已硬编码），通过后把 `license` + `sig` 原样落盘
（建议 `%APPDATA%\com.indmak.rarzap\license.json` / macOS `~/Library/Application Support/com.indmak.rarzap/` / Linux `~/.local/share/rarzap/`）。

失败：

| HTTP | error | 含义 | 客户端动作 |
| --- | --- | --- | --- |
| 400 | `invalid_params` | 参数缺失/超长 | 提示检查输入 |
| 403 | `invalid_license` | key 不存在 / 已吊销（含退款、拒付） | 提示联系 support@indmak.com |
| 403 | `device_limit_reached` | 1 台名额满（响应附 `max`） | 引导解绑旧设备（§4.3）或联系客服 |
| 429 | `rate_limited` | 限流 | 退避重试 |
| 500 | `license_sign_failed` 等 | 服务端签名故障 | 上报（不应发生） |

### 2.1 刷新离线文件（同一接口）

离线文件 `exp` = 签发 + **60 天**。**每 7 天**或 `exp` 前 7 天，用同一 key + 同一指纹再调一次 `/activate` 拿新文件（`rebound: true`，不占名额）。不要等过期才刷。

### 2.2 什么时候该调 /activate

| 时点 | 动作 |
| --- | --- |
| 用户输入激活码点激活 | `/activate` → 验签落盘 → Licensed |
| 启动时文件 `exp` 距今 < 7 天且有网 | 后台静默 `/activate` 刷新 |
| 换机迁移 | 旧机 `DELETE /deactivate` → 新机 `/activate` |

---

## 3. 离线许可文件验签（引擎端必实现）

1. 按**固定顺序**拼规范串（无空格、无换行）：

   ```text
   v={v}|kid={kid}|key={key}|fp={fp}|iat={iat}|exp={exp}|max={max}|ps={ps}|pe={pe}|features={csv}
   ```

   `{csv}` = `features` 数组逗号拼接（空数组 = 空串）。RarZap 买断码实例：

   ```text
   v=1|kid=2026-09-B|key=IND-0EEEE4-83A979-034D591A|fp=sha256:9f2c…|iat=1791000000|exp=1796184000|max=1|ps=0|pe=0|features=
   ```

2. `sig` base64url 解码（补 `=` 填充）→ 64 字节。
3. Ed25519 验签。**验签通过后必须再检查**：
   - `fp` == 本机当前指纹（防拷贝）；
   - `now < exp`（离线文件过期 → 联网刷新；刷不到则锁批量功能）；
   - `pe > 0` 时 `now <= pe`（买断码 `pe=0` 跳过）；
   - `kid` 白名单 = `{"2026-09-B"}`，未知 kid → 判无效并提示更新客户端。
4. 验签通过的 `{license, sig}` 整体落盘，不要拆开重组。

公钥（与 `https://www.indmak.com/api/licenses/pubkey` 一致，**硬编码进二进制**）：

```json
{ "kty": "OKP", "crv": "Ed25519", "x": "a1vEewAyfr_gTikC3NJrcw0GF0fXHStOnCNIuirSIaQ" }
```

---

## 4. 在线校验 / 解绑 / 权益

### 4.1 周期校验

```http
POST https://www.indmak.com/api/licenses/verify
Content-Type: application/json

{ "licenseKey": "IND-0EEEE4-83A979-034D591A", "deviceFingerprint": "sha256:9f2c…" }
```

也接受 `Authorization: Bearer <licenseKey>`（body 可省 `licenseKey`）。

```jsonc
{ "valid": true, "kind": "one_time", "status": "active", "periodEnd": null, "features": [] }
```

- `periodEnd: null` = 无期限（买断）。节奏：**有网每 24h 一次**；一次失败不锁功能（弱网常见），连续失败且非网络错误才锁批量功能。
- 旧 `GET …/verify?key=…` 为过渡兼容，**新客户端勿用**。

### 4.2 权益（插件模块，预留）

```http
POST https://www.indmak.com/api/licenses/entitlements
Content-Type: application/json

{ "licenseKey": "…", "deviceFingerprint": "sha256:9f2c…" }
```

响应含 `features: ["sku", …]` + `addons`。RarZap 当前无付费插件，`features` 恒为空 —— 客户端照常实现即可。

### 4.3 解绑

```http
DELETE https://www.indmak.com/api/licenses/deactivate
Content-Type: application/json

{ "licenseKey": "…", "deviceFingerprint": "sha256:9f2c…" }
```

```jsonc
{ "ok": true, "freed": true }   // freed=false = 本来就没绑定（幂等，安全重试）
```

---

## 5. 机器指纹

- 在**引擎侧**取 machine id，`fp = "sha256:" + hex(SHA256(machine_id_raw))`；**不要原文传输**。
- ≤256 字符；Windows 取 MachineGuid，macOS 取 IOPlatformUUID，Linux 取 `/etc/machine-id`。
- 重装系统可能变化（正常，走解绑+重激活）。

---

## 6. 版本与下载

- **开源版**：从 `https://github.com/indmak/RarZap/releases` 下载（Windows `.exe` / macOS `.dmg` / Linux `.deb`）。**不经本站下载。**
- **商业版**：经**微软商店**（Windows）分发（上架前预留链接）；本站 `/api/software/latest?slug=rarzap` 目前返回 `latest:null`（我们不自托管该安装包）。
- 若将来改为本站分发，接口与其它产品一致：`GET /api/software/latest?slug=rarzap&platform=…` 与 `GET /api/downloads/latest?slug=rarzap&platform=…`（无版返回 `404 no_release`），下载走 `/api/downloads/file/<id>`（支持 Range）。

---

## 7. 购买入口

**不要直链 Creem 收银台**，用稳定入口（登录可预填邮箱、激活码自动进用户中心）：

| 场景 | 地址 |
| --- | --- |
| 通用购买入口（推荐） | `https://www.indmak.com/buy/rarzap` |
| 预填邮箱 | `https://www.indmak.com/products/rarzap?email=<URL编码邮箱>` |
| 只打开产品页 | `https://www.indmak.com/products/rarzap` |
| 微软商店 | 上架后由官网/应用内链接过去（预留） |

付款方式：Card · Apple Pay · Google Pay（Creem 作为 MoR，含税）。支付完成 → webhook 自动签发激活码 + 邮件送达。
激活码兜底：订购邮件 / 用户中心 My licenses / 客服补发（`support@indmak.com`）。

---

## 8. 引擎接口建议（许可判定层）

```rust
pub enum AccessState {
    Licensed,        // 商业版已激活（pe=0 终身）
    Unlicensed,      // 商业版未激活
    Free,            // 开源版（无许可需求）
}
pub fn check_access() -> AccessState;   // 读许可文件 → §3 验签 → 无文件则 Unlicensed
pub fn activate(code: String) -> Result<AccessState, String>;   // §2
pub fn deactivate() -> Result<bool, String>;                    // §4.3
```

- 公钥、`kid` 白名单是**引擎常量**，UI 不可见。
- 激活码只存受保护设置键，**不进日志与崩溃上报**。
- **开源版不包含任何付费判断代码**（见架构：CORE 定义插件接口，商业版通过接口扩展）。商业版未激活时：单文件转换仍可用，**仅批量引擎锁定**。

---

## 9. 联调清单

1. **激活**：测试码 `POST /activate` → 验签（§3）通过；同指纹再激活 → `rebound:true` 且 `seats.used` 不变 ✅。
2. **名额**：第二台设备激活 → `403 device_limit_reached`（上限 1）；旧机 `DELETE /deactivate` 后可激活 ✅。
3. **离线**：激活后断网重启 → 验签放行；改系统时间到 `exp` 之后 → 锁批量 ✅。
4. **无试用（硬拒绝）**：`POST /api/trials/start` 与 `/api/trials/status`（`slug=rarzap`）均返回 `400 {"error":"trial_not_offered"}`；`/activate` 响应 `trial` 恒为 `null` ✅。
5. **吊销**：退款 key 走 `/verify` → `valid:false` → 锁批量但不删数据 ✅。
6. **开源版**：不产生任何许可请求（纯离线、无 key）✅。

---

## 10. 变更记录

- 2026-10-08 v1.0：初版。RarZap 专用（slug `rarzap`，商业版买断 $9，Creem `prod_NZq1928pUasZRXcgzBHl1`，**1 台设备**，**无试用期**；开源版 MIT 免费经 GitHub 分发，商业版经微软商店 + 激活码）。契约与《LICENSING-API.md》v1.12 同源（slug 必填、按产品分桶、离线文件 60 天）。服务端已上线产品级 **`trial_disabled`** 开关，RarZap 的 `/api/trials/*` 返回 `400 trial_not_offered`。
