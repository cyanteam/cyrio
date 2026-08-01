# Diamond Rio S-Series USB 协议规范

> **目标机型**：Rio S10 / S30S / S35S / S50（firmware generation 4）
> **USB Vendor ID**：`0x045a`（Diamond / DNNA — Digital Networks North America）
> **协议性质**：自定义 USB 协议，**非** USB Mass Storage。原厂通过 Windows 内核驱动
> `RIOUNIV.SYS` (v2.4.0.22) 暴露为 WMDM (Windows Media Device Manager) Service Provider。
> **本规范来源**：逆向开源实现 [rioutil](https://github.com/hjelmn/rioutil) v1.5.3 源码
> （作者 Nathan Hjelm），与原厂驱动 INF (`riouniv.inf`) 交叉验证。

---

## 目录

1. [USB 物理层](#1-usb-物理层)
2. [命令格式](#2-命令格式)
3. [操作码全集](#3-操作码全集)
4. [握手包格式](#4-握手包格式)
5. [CRC32 算法](#5-crc32-算法)
6. [设备初始化序列](#6-设备初始化序列)
7. [上传文件时序](#7-上传文件时序)
8. [下载文件时序](#8-下载文件时序)
9. [删除文件时序](#9-删除文件时序)
10. [格式化内存时序](#10-格式化内存时序)
11. [rio_file_t 结构体字节布局](#11-rio_file_t-结构体字节布局)
12. [rio_mem_t 结构体字节布局](#12-rio_mem_t-结构体字节布局)
13. [FIDL/ST10 播放列表格式](#13-fidlst10-播放列表格式)
14. [内存单元与 SD 卡](#14-内存单元与-sd-卡)
15. [MP3 文件要求](#15-mp3-文件要求)
16. [已知陷阱（Gotchas）](#16-已知陷阱gotchas)
17. [错误处理与中止](#17-错误处理与中止)
18. [参考文献](#18-参考文献)
19. [附录 A：原厂资料交叉验证](#附录-a原厂资料交叉验证)

---

## 1. USB 物理层

### 1.1 设备识别

| 机型   | idVendor | idProduct | DeviceGUID（INF 原厂）                          | 备注                                   |
|--------|----------|-----------|--------------------------------------------------|----------------------------------------|
| Rio S10  | `0x045a` | `0x5005` | `{54413cef-ac05-4ff9-a009-9678eb4ec541}`         |                                        |
| Rio S50  | `0x045a` | `0x5006` | `{dc15545f-a470-429b-b1aa-8d45f4168091}`         | 用户当前设备（S30S 刷 S50 固件后报告此 PID） |
| Rio S35S | `0x045a` | `0x5007` | `{5906da6d-df99-4038-b99f-f2309ea7db94}`         |                                        |
| Rio S30S | `0x045a` | `0x5009` | `{d90410c4-bc73-4543-b6f6-0172b477ccbb}`         | 用户硬件原始 PID                          |

**关键**：刷固件后 USB PID 由固件决定。用户的 S30S 刷了 S50 固件，所以会报告
PID=`0x5006`。这四款机型共享同一代固件（generation 4）和同一套协议，端点布局完全一致。

**DeviceGUID 来源**：原厂驱动 `riouniv.inf` v2.4.0.22 中各机型的 `DeviceGUID` 注册表项，
Windows 通过此 GUID 把设备暴露给 WMDM (Windows Media Device Manager) Service Provider。
详见[附录 A.1](#a1-riounivinf-设备-guid-表)。

### 1.2 端点布局

S-Series 是 USB 1.1 Full-Speed 设备（12 Mbps），单配置单接口，3 个端点：

| 端点地址 | 类型     | 方向 | 用途                              | Max Packet |
----------|----------|------|-----------------------------------|------------
| `0x00`   | Control  | 双向 | Vendor Control Transfer（发命令） | 64B        |
| `0x81`   | Bulk     | IN   | 设备 → 主机（响应、文件数据下载）  | 64B        |
| `0x02`   | Bulk     | OUT  | 主机 → 设备（命令包、文件数据上传）| 64B        |

**注意**：端点地址的 bit7 表示方向（1=IN, 0=OUT）。`0x81` = 端点 1 IN，`0x02` = 端点 2 OUT。

**与老机型差异**：Rio 600/800/900（generation 3）使用端点 2 双向（IN=`0x82`, OUT=`0x02`）。
S-Series 改为 IN=`0x81`/OUT=`0x02`，这是两代协议的主要物理差异。

### 1.3 接口 claim

通信前必须：
1. `libusb_set_configuration(dev, 1)`
2. `libusb_claim_interface(dev, 0)`

macOS 上 Rio 设备无内核驱动，可直接 claim，**无需 sudo**。
Linux 上若未加载 `rio500` 内核驱动（gen3 专用，且已在 v4.19 移除），同样可直接 claim。

---

## 2. 命令格式

所有命令通过 **USB Vendor Control Transfer** 发送：

| 字段            | 值                  | 说明                          |
|-----------------|---------------------|-------------------------------|
| `bmRequestType` | `0xC0`              | Vendor Request, Device→Host, IN |
| `bRequest`      | opcode              | 见[§3](#3-操作码全集)         |
| `wValue`        | arg1 (16-bit LE)    | 通常为 memory_unit            |
| `wIndex`        | arg2 (16-bit LE)    | 通常为 file_no 或 0           |
| `wLength`       | `0x000C` (12)       | 状态缓冲区长度                |
| 数据阶段        | 12 字节状态缓冲区   | `status[0]==0x01` 表示成功    |

**成功判定**：返回的 12 字节缓冲区首字节必须为 `0x01`。否则重试最多 3 次（每次间隔 50ms），
仍失败则判定设备不可用。

**超时**：15 秒（与 rioutil 一致）。

---

## 3. 操作码全集

| Opcode | 名称          | arg1 (wValue)    | arg2 (wIndex)    | 读/写     | 说明                          |
|--------|---------------|------------------|------------------|-----------|-------------------------------|
| `0x60` | UNKNOWN00     | 0                | 0                | -         | 初始化握手 / 上传完成收尾      |
| `0x61` | RIO_POLLD     | 0                | 0                | -         | 轮询设备（init 中连发 2 次）   |
| `0x62` | RIO_DESCP     | -                | -                | 读 256B   | 设备描述（固件版本/序列号/型号）|
| `0x63` | RIO_TYPEQ     | type_index (0-2) | 0                | 读 64B×2  | 查询支持的文件类型             |
| `0x65` | UNKNOWN65     | 0                | 0                | -         | init 中发送一次，目的不明       |
| `0x68` | RIO_MEMRI     | memory_unit      | -                | 读 256B   | 内存单元信息                   |
| `0x69` | RIO_FILEI     | memory_unit      | file_no          | 读 2048B  | 文件信息（rio_file_t）         |
| `0x6a` | RIO_FORMT     | memory_unit      | -                | 读 64B×N  | 格式化内存单元                 |
| `0x6b` | RIO_UPDAT     | 0x1              | 0=Fim / 1=Rom    | -         | 固件/ROM 升级（本库不实现）    |
| `0x6c` | RIO_WRITE     | memory_unit      | 0                | 见[§7](#7-上传文件时序) | 上传新文件（设备分配新 file_no）|
| `0x70` | RIO_READF     | memory_unit      | 0                | 见[§8](#8-下载文件时序) | 下载文件                       |
| `0x78` | RIO_DELET     | memory_unit      | 0                | 见[§9](#9-删除文件时序) | 删除文件                       |
| `0x79` | RIO_PREFS     | -                | -                | 写 2048B  | 写设备偏好                     |
| `0x7a` | RIO_PREFR     | -                | -                | 读 2048B  | 读设备偏好                     |
| `0x7b` | RIO_TIMES     | time >> 16       | time & 0xffff    | -         | 设置设备时钟（Unix 时间戳）    |
| `0x7c` | RIO_GIDS      | ?                | ?                | 读 64B    | Group ID（本库不用）           |
| `0x85` | RIO_CHGIN     | memory_unit      | file_no          | -         | 修改文件元信息（S-Series+）    |
| `0x87` | RIO_NINFO     | 0                | 0                | 写 16KB×N | Nitrus 数据库更新（本库不用）  |
| `0x88` | RIO_OVWRT     | memory_unit      | 已存在 file_no   | 见[§7](#7-上传文件时序) | 覆盖已存在文件                 |

### 关于 RIO_READF (0x70) 的重要说明

rioutil 的 `rioi.h` 中此 opcode 注释为 `/* doesnt work on S-Series */`，但这是**过时**信息。
`song_management.c` 第 969-971 行明确说明：

> "All of the newer players from Rio support the download of any file on the player!"

S-Series 属于 generation 4，原生支持下载任意文件，且**不会**删除原文件。
老机型（generation 3: Rio 600/800/900）下载受 `bits[7]` 标志限制，且可能删除原文件——
本库不涉及老机型。

---

## 4. 握手包格式

所有握手包都是 **64 字节**，结构如下：

```
偏移  长度  字段
0x00  8     ASCII 魔数（如 "CRIODATA"、"SRIORDY"），不足 8 字节时末尾补 0
0x08  4     CRC32（**大端**；仅 CRIODATA 有；CRIOINFO 为 0x00000000；设备响应包此字段无意义）
0x0C  52    0 填充
```

### 4.1 主机 → 设备（Host → Device）

| 魔数       | 含义                                       | 后接数据           |
|------------|--------------------------------------------|--------------------|
| `CRIODATA` | 数据块前导，含 CRC32 校验                   | 16384B 数据块      |
| `CRIOINFO` | 文件头前导，**无 CRC**（4 字节恒为 0）      | 2048B rio_file_t   |
| `CRIOABRT` | 中止传输                                   | 无                 |

### 4.2 设备 → 主机（Device → Host）

| 魔数       | 含义                                                     |
|------------|----------------------------------------------------------|
| `SRIORDY`  | 设备就绪。上传开始时设备先发此包                          |
| `SRIODATA` | 数据块确认。主机每上传一块后设备回此包                    |
| `SRIODONE` | 传输完成。下载循环中设备发此包表示文件已全部发送           |
| `SRIONOFL` | 文件不存在。下载时若 file_no 无效，设备回此包             |
| `SRIODELS` | 删除开始确认。发 DELET 命令后设备回此包                   |
| `SRIODELD` | 删除完成。主机发送要删除的文件头后设备回此包              |
| `SRIOFMTD` | 格式化完成（可能附带 "...Done" 文本）                     |
| `SRIOPR<n>`| 格式化进度报告（`<n>` 为进度百分比数字）                  |

---

## 5. CRC32 算法

CRIODATA 包的 bytes[8..11] 存放其所后接数据块的 CRC32 校验值（**大端** u32）。

rioutil 的 `cksum.c` 中 `crc32_rio` 使用**非标准** CRC32 变体，与标准 ZIP CRC32 不同：

- **多项式**：`0x04C11DB7`（非反射形式，但表用右移构建 — rioutil 特殊行为）
- **初始值**：`0`
- **输入/输出反射**：是（表构建用右移 + LSB 检测）
- **异或输出**：无
- **字节序**：**大端**（rioutil `big32_2_arch32` 宏转换后写入）

**与标准 ZIP CRC32 的区别**（标准 ZIP：poly `0xEDB88320`、init `0xFFFFFFFF`、final XOR `0xFFFFFFFF`）：
- 真机实测：设备**只接受** rioutil 算法 + 大端字节序的 CRIODATA 包
- 标准 ZIP CRC32 会被设备拒绝

**测试向量**：
```
crc32("123456789") == 0x0328b978    // rioutil 算法
crc32(全零 16384B)   == 0x00000000    // 下载场景下 CRIODATA 的 CRC 总为 0
```

**注意**：CRIOINFO 包**不**计算 CRC，bytes[8..11] 恒为 `0x00000000`。这是协议中最常被
实现错误的点——许多初版实现误把 rio_file_t 的 CRC32 写入 CRIOINFO，导致设备拒绝。

Node.js 实现见 `src/protocol/crc32.ts`，自包含 256 项预计算表，无外部依赖。

---

## 6. 设备初始化序列

打开设备后必须执行以下序列（来自 rioutil `rio.c` 的 `open_rio`）：

```
1. libusb_set_configuration(dev, 1)
2. libusb_claim_interface(dev, 0)
3. send_command(0x60, 0, 0)                          // 握手
4. send_command(0x7b, time>>16, time&0xffff)         // 设置时钟（本地时间）
5. send_command(0x61, 0, 0)                          // poll
6. send_command(0x61, 0, 0)                          // poll
7. send_command(0x65, 0, 0)                          // unknown but required
8. for i in 0..2:                                    // 查询 3 种文件类型
     send_command(0x60, 0, 0)
     send_command(0x63, i, 0)
     read_block(64)                                  // 丢弃
     read_block(64)                                  // 丢弃
9. unlock_rio                                         // 释放内部锁
```

**时钟设置说明**：设备无时区概念。应传本地时间对应的 Unix 时间戳。
rioutil 的实现：`curr_time = tv.tv_sec - 60 * tz.tz_minuteswest`，并按夏令时调整。

---

## 7. 上传文件时序

上传使用 `RIO_WRITE (0x6c)`（新文件）或 `RIO_OVWRT (0x88)`（覆盖已存在文件），
两者时序相同，仅 opcode 和 wIndex 不同：

- `RIO_WRITE`：`wIndex=0`，设备分配新 file_no
- `RIO_OVWRT`：`wIndex=已存在 file_no`，原地覆盖

```
主机                                          设备
  |                                            |
  | send_command(0x6c, memory_unit, 0)  ---->  |   开始上传
  | <--- bulk IN  64B  "SRIORDY"               |   设备就绪
  | <--- bulk IN  64B  "SRIODATA"              |   等待数据
  |                                            |
  |  循环（每次 16384B，直到文件传完）:         |
  |   bulk OUT 64B  "CRIODATA" + CRC32(chunk)  |
  |   bulk OUT 16384B  (数据块)                |
  |   <--- bulk IN  64B  "SRIODATA"            |   块确认
  |                                            |
  | bulk OUT 64B  "CRIOINFO" + 0x00000000      |   文件头前导（无 CRC！）
  | bulk OUT 2048B  (rio_file_t)               |   文件头
  | <--- bulk IN  64B  "SRIODATA"              |   头确认
  |                                            |
  | send_command(0x60, 0, 0)            ---->  |   收尾
  |                                            |
```

**关键细节**：
- 最后一块数据若不足 16384B，仍按 16384B 发送（不足部分补 0）
- CRIOINFO 的 CRC32 字段恒为 `0x00000000`，**不**计算 rio_file_t 的 CRC
- 上传完成后必须发 `send_command(0x60, 0, 0)` 收尾，否则设备状态可能异常

---

## 8. 下载文件时序

下载使用 `RIO_READF (0x70)`。S-Series 原生支持，详见[§3](#关于-rio_readf-0x70-的重要说明)。
时序参考 rioutil `song_management.c` 的 `download_file_rio`（第 972-1154 行）。

### 8.1 完整时序

```
主机                                          设备
  |                                            |
  |  步骤 1: slot 查找（获取完整文件头）        |
  |   for slot in 0..N:                        |
  |     send_command(0x69, memUnit, slot) -->  |   RIO_FILEI
  |     <--- bulk IN  2048B  (rio_file_t)      |   返回该槽位的文件头
  |       if file_no == 0: 空槽，文件不存在    |
  |       if file_no == target: 保存此 buffer  |
  |                                            |
  | send_command(0x70, memory_unit, 0)  ---->  |   开始下载
  | <--- bulk IN  64B  (SRIOUPLD)              |   初始响应
  |                                            |
  | bulk OUT 2048B  (完整 rio_file_t)          |   步骤 1 获取的完整文件头
  | <--- bulk IN  64B                          |   响应
  |   if "SRIONOFL": 文件不存在，中止          |
  |                                            |
  |  步骤 6: 数据块循环（blocks = ceil(size/16384)）
  |   bulk OUT 64B  "CRIODATA" + CRC32(空块)   |
  |   <--- bulk IN  64B                        |
  |     if "SRIODONE": 传输完成，break         |
  |   <--- bulk IN  16384B  (文件数据块)       |
  |                                            |
  |  步骤 7: 若循环未收到 SRIODONE             |
  |   bulk OUT 64B  "CRIODATA"                 |   gen4+ 不读响应
  |                                            |
```

### 8.2 关键细节

- **步骤 1（slot 查找）**：`RIO_FILEI` 的 `wIndex` 是 **0-based 槽位索引**（真机实测），
  不是真实文件号。设备返回的 `rio_file_t.file_no`（offset 0x00）才是真实文件号。
  必须先迭代 slot=0,1,2,... 找到 `file_no` 匹配的槽位，获取完整 2048B 文件头 buffer。
- **步骤 4（写完整文件头）**：必须写入步骤 1 获取的**完整** 2048B 文件头 buffer，
  而非空 rio_file_t + fileNo。rioutil `download_file_rio` 先调 `get_file_info_rio`
  获取完整 `rio_file_t`，然后写入。写空文件头会导致设备返回 SRIOCOMM 错误。
- **步骤 6（循环次数）**：`blocks = ceil(size / 16384)`，循环固定 blocks 次。
  每次循环：发 CRIODATA → 读 64B 响应（SRIODONE 则提前 break）→ 读 16384B 数据块。
  CRIODATA 含空 16384B 块的 CRC32（全零块 CRC=0）。
- **步骤 7（循环结束）**：若循环结束后仍未收到 SRIODONE，再发一个 CRIODATA。
  **gen4+（S-Series）不读响应**（rioutil 仅对 gen3 读 64B 响应）。
- 数据块大小：S-Series 为 16384B，老机型为 4096B
- 下载得到的是**纯音频数据**（无 rio_file_t 头），可直接保存为 .mp3
- 若上传时跳过了 ID3v2，下载结果也不含 ID3v2

---

## 9. 删除文件时序

删除使用 `RIO_DELET (0x78)`。

```
主机                                          设备
  |                                            |
  | send_command(0x78, memory_unit, 0)  ---->  |   开始删除
  | <--- bulk IN  64B  "SRIODELS"              |   删除就绪
  |                                            |
  | bulk OUT 2048B  (rio_file_t, 含 file_no)   |   指定要删除的文件
  |                                            |
  | <--- bulk IN  64B  "SRIODELD"              |   删除完成
  |                                            |
```

**关键细节**：
- 删除时发送的 2048B rio_file_t **不**前置 CRIODATA 包，直接写裸 2048B
  （这是与上传 CRIOINFO 阶段的关键区别）
- rio_file_t 中只需 `file_no` 字段正确，其他字段可忽略
- 删除后应更新本地文件列表缓存

---

## 10. 格式化内存时序

格式化使用 `RIO_FORMT (0x6a)`。

```
主机                                          设备
  |                                            |
  | send_command(0x6a, memory_unit, 0)  ---->  |   开始格式化
  |  循环 read_block(64):                       |
  |   <--- bulk IN  64B  "SRIOPR<n>"           |   进度报告（n=百分比）
  |   ...                                      |
  |   <--- bulk IN  64B  "SRIOFMTD...Done"     |   格式化完成
  |                                            |
```

格式化会擦除该内存单元上的所有文件。`memory_unit=1` 可格式化 SD 卡。

---

## 11. rio_file_t 结构体字节布局

每个文件在设备上由一个 **2048 字节**的头描述。通过 `RIO_FILEI (0x69)` 读取，
通过 `CRIOINFO` 阶段写入。所有多字节字段为**小端**。

```
偏移    长度  类型      字段名          说明
0x0000  4     u32 LE    file_no         文件编号（1-based，0=空槽）
0x0004  4     u32 LE    start           文件数据起始偏移（0=设备分配）
0x0008  4     u32 LE    size            文件大小（字节）
0x000C  4     u32 LE    time            时长（秒）
0x0010  4     u32 LE    mod_date        修改时间（Unix 时间戳）
0x0014  4     u32 LE    bits            标志位（见下表）
0x0018  4     u32 LE    type            文件类型 FourCC（见下表）
0x001C  4     u32 LE    foo3            未知
0x0020  4     u32 LE    foo4            未知（可能是声道数）
0x0024  4     u32 LE    sample_rate     采样率（Hz，如 44100）
0x0028  4     u32 LE    bit_rate        比特率（kbps × 128，即 << 7）
0x002C  4     u32 LE    foo5            未知
0x0030  48    bytes     foobar          通常为 0
0x0060  16    bytes     info0           未知
0x0070  8     bytes     unk             未知
0x0078  4     bytes     unk1            S-Series 播放列表关联
0x007C  4     bytes     unk2            未知
0x0080  64    bytes     info1           未知
0x00C0  64    char[64]  name            文件名（latin1 + NUL 填充）
0x0100  64    char[64]  title           标题
0x0140  64    char[64]  artist          艺术家
0x0180  64    char[64]  album           专辑
0x01C0  ...   ...       ...             其余字段（RIOT 专用，S-Series 不用）
...
0x07FF  (end)
```

### bits 标志位

| Bit       | 含义                                        |
|-----------|---------------------------------------------|
| `0x00000001` | **必须置位**（与下面两位组合）           |
| `0x00000010` | **必须置位**                              |
| `0x00000002` | 有书签                                    |
| `0x00000080` | 可下载（gen3 需要；S-Series 全部可下载）  |
| `0x00000100` | **必须置位**                              |
| `0x00400000` | 文件名不在设备界面显示                    |

**强制规则**：序列化时必须 `bits |= 0x00000001 | 0x00000010 | 0x00000100`（即 `0x111`），
否则设备拒绝文件。

### type FourCC

| FourCC (ASCII) | u32 LE 值     | 含义        |
|----------------|---------------|-------------|
| `MPG3`         | `0x4D504733`  | MP3 文件    |
| `WMA `         | `0x20414D57`  | WMA 文件    |
| `ACLP`         | `0x504C4341`  | WAV 文件    |
| `WAVE`         | `0x45564157`  | WAV 文件（备用）|
| `PLS `         | `0x504C5320`  | 播放列表    |

### bit_rate 字段

**关键**：`bit_rate` 字段存储的是 `实际比特率_kbps × 128`（即 `<< 7`）。

| 实际比特率 | 存储值    |
|-----------|-----------|
| 64 kbps   | 8192      |
| 128 kbps  | 16384     |
| 320 kbps  | 40960     |
| VBR       | 平均比特率 × 128 |

---

## 12. rio_mem_t 结构体字节布局

通过 `RIO_MEMRI (0x68)` 读取，描述一个内存单元。共 **256 字节**。

```
偏移    长度  类型      字段名     说明
0x0000  16   u32[4]    foo        保留
0x0010  4    u32 LE    size       总大小（S-Series 单位为**字节**）
0x0014  4    u32 LE    used       已用（字节）
0x0018  4    u32 LE    free       空闲（字节）
0x001C  4    u32 LE    system     系统保留（字节）
0x0020  32   u32[8]    foobar     保留
0x0040  64   char[64]  name       内存单元名（如 "Internal Memory"）
0x0080  32   bytes     unk0       未知
0x00A0  32   bytes     unk1       未知
0x00C0  64   char[64]  model      型号字符串
0x0100  16   bytes     unk        未知
...
0x00FF  (end)
```

**关键**：S-Series 的 size/used/free 字段单位是**字节**，显示时 `/1024/1024` 得 MB。
老机型（Riot）此字段单位是 KB——本库仅支持 S-Series，按字节处理。

---

## 13. FIDL/ST10 播放列表格式

播放列表作为 `type=TYPE_PLS (0x504C5320)` 的文件存储在设备上。文件内容是 FIDL/ST10
二进制格式，记录播放列表中包含的歌曲 rio_num 列表。

### 13.1 文件结构

```
偏移  长度  字段
0x00  4     "FIDL" (0x46 0x49 0x44 0x4C)        魔数
0x04  2     "ST"   (0x53 0x54)                  子类型
0x06  1     0x01                               主版本号
0x07  1     0x00                               次版本号
0x08  1     0x00                               保留
0x09  3     nsongs (24-bit Little-Endian)      歌曲数量
0x0C  N×6   条目数组                            每条 6 字节
```

### 13.2 条目结构（每条 6 字节）

```
偏移  长度  字段
0x00  3     rio_num (24-bit LE)   歌曲在设备上的内部文件编号
0x03  3     sflags (3 bytes)       标志位（仅第 3 字节有意义）
```

### 13.3 修改播放列表工作流

要向播放列表追加歌曲：
1. 用 `RIO_READF (0x70)` 下载播放列表文件内容 → `Buffer`
2. 解析 FIDL/ST10 → 得到 `rio_num` 数组
3. 追加新条目 `{ rioNum: songFileNo, flags: 0 }`
4. 重新序列化为 FIDL/ST10 → `newBytes`
5. 用 `RIO_OVWRT (0x88)` 原位覆盖播放列表文件（`wIndex=播放列表的 file_no`）

---

## 14. 内存单元与 SD 卡

S-Series 设备最多有 2 个内存单元：

| memory_unit | 含义        | S50 容量    |
|-------------|-------------|-------------|
| 0           | 内置闪存    | 128 MB      |
| 1           | SD/MMC 卡   | 取决于插入的卡 |

- 若未插 SD 卡，`RIO_MEMRI(1)` 返回 `size=0`
- 所有文件操作（upload/download/delete/list）都接受 `memory_unit` 参数
- 文件编号在每个内存单元内独立（unit 0 的 file_no=1 与 unit 1 的 file_no=1 是不同文件）

---

## 15. MP3 文件要求

### 15.1 支持的格式

| 项           | 支持值                                |
|--------------|---------------------------------------|
| 格式         | MP3 (MPEG-1/2 Layer III), WMA, WAV   |
| 比特率       | 32–320 kbps CBR，支持 VBR             |
| 采样率       | 32 / 44.1 / 48 kHz（MPEG-1）         |
|              | 16 / 22.05 / 24 kHz（MPEG-2）        |
| 声道         | 单声道 / 立体声                       |
| ID3 标签     | ID3v1 支持，ID3v2 上传时**必须跳过**  |

### 15.2 ID3v2 跳过

设备固件不能正确解析 ID3v2 标签。上传时必须从 MP3 帧数据开始处读取（跳过 ID3v2），
即 `info.skip = id3v2Size`。

ID3v2 大小计算（syncsafe 7-bit 编码）：
```
id3v2Size = 10 + ((header[6] & 0x7f) << 21)
              | ((header[7] & 0x7f) << 14)
              | ((header[8] & 0x7f) << 7)
              | (header[9] & 0x7f)
```

### 15.3 SDMI/DRM

S50 **不强制** SDMI 包装。`bits & 0x80` 标志位控制"可下载"属性，但 S-Series 全部文件
均可下载，无需设置此位。

---

## 16. 已知陷阱（Gotchas）

以下是实现时最容易踩的 12 个坑，逐一列出：

### 16.1 字节序

USB 线上传输的所有多字节整数均为**小端**。Node.js 的 `Buffer.readUInt32LE` /
`writeUInt32LE` 必须显式写 `LE`，禁止用 `readUInt32BE` 或依赖默认。
Rust 端用 `from_le_bytes` / `to_le_bytes`。

**唯一例外**：CRIODATA 包的 bytes[8..11] CRC32 字段用**大端**字节序
（rioutil `big32_2_arch32` 宏）。详见 [§5](#5-crc32-算法)。

### 16.2 CRIOINFO 无 CRC

`CRIOINFO` 包的 bytes[8..11] 恒为 `0x00000000`，**不**计算 rio_file_t 的 CRC32。
这是协议中最常被实现错误的点。只有 `CRIODATA` 包才带 CRC。
**CRIODATA 的 CRC 用 rioutil 非标准算法 + 大端字节序**，详见 [§5](#5-crc32-算法)。

### 16.3 Delete 发送裸 2048B

`RIO_DELET` 流程中，写 rio_file_t 头时**不**前置 CRIODATA 包，直接写 2048B 原始头。
这与上传的 CRIOINFO 阶段不同——上传时 rio_file_t 前必须有 CRIOINFO 包。

### 16.4 S-Series 支持 RIO_READF

rioi.h 中 `RIO_READF 0x70 /* doesnt work on S-Series */` 注释是**过时**的。
song_management.c 969-971 行明确确认 S-Series 及更新机型支持下载。
本库直接使用，无需特殊处理。

### 16.5 bit_rate = kbps << 7

`rio_file_t.bit_rate` 字段存储的是 `实际比特率_kbps × 128`，不是直接存 kbps。
128kbps 歌曲写入 `16384`，320kbps 写入 `40960`。

### 16.6 内存字段单位是字节

S-Series 的 `rio_mem_t.size/used/free` 字段单位是**字节**，不是 KB。
显示时 `/1024/1024` 得 MB。老机型（Riot）才是 KB——本库不涉及。

### 16.7 ID3v2 上传时必须跳过

上传 MP3 时，从 `id3v2Size` 偏移开始读音频数据，否则设备播放会爆音或静音。
`info.skip = id3v2Size`，块读取时 `lseek(fd, info.skip, SEEK_SET)`。

### 16.8 send_command 成功判定

返回的 12 字节状态缓冲区首字节必须 `=== 0x01`。否则重试 3 次（每次间隔 50ms）。
注意：`0x66` 和 `0x61` 命令的响应不遵循此规则（rioutil 中特判）。

### 16.9 块大小

S-Series 数据块 **16384B**，握手包 **64B**，rio_file_t 头 **2048B**。
老机型数据块 4096B——本库 S-Series 硬编码 16384。

### 16.10 OVWRT 的 wIndex 是已存在文件号

`RIO_OVWRT (0x88)` 的 `wIndex` 传**已存在文件号**，不是 0。
`RIO_WRITE (0x6c)` 的 `wIndex` 才是 0（让设备分配新号）。
混淆会导致设备状态异常。

### 16.11 字符串编码 latin1

`rio_file_t` 的 name/title/artist/album 是 **latin1 + NUL 填充**，**不是** UTF-8。
中文歌曲名会丢失字符（显示为 `?`），这是设备固件限制，非本库 bug。
Node.js 用 `buffer.toString('latin1')` / `buffer.write(str, 'latin1')`。

### 16.12 bits 必须置位 0x111

`rio_file_t.bits` 的 `0x01 | 0x10 | 0x100` 三位**必须同时置位**，否则设备拒绝文件。
序列化时强制 `bits |= 0x111`。这三位的精确含义 rioutil 未明确，但实测必须置位。

---

## 17. 错误处理与中止

### 17.1 命令重试

`send_command` 失败（`status[0] !== 0x01`）时重试最多 3 次，每次间隔 50ms。
仍失败则抛 `RioCommandError`。

### 17.2 异常中止

任何数据传输阶段捕获到异常时，**尽力**发送 64B `"CRIOABRT"` 包（不抛其错误的二次错误），
然后才抛原始异常。这能避免设备卡在等待数据状态。

### 17.3 超时

- Control Transfer：15 秒
- Bulk Transfer：8 秒（与 rioutil 一致）
- 超时后抛 `RioTimeoutError`

### 17.4 不期望的响应

收到非预期的魔数（如下载时收到非 `SRIODATA`/`SRIODONE`/`SRIONOFL` 的字符串）时，
立即中止并抛 `RioProtocolError`，不尝试"继续读"。

### 17.5 USB 错误

`LIBUSB_ERROR_*` 直接包装为 `RioUsbError(code, message)`，不重试（设备级故障重试无意义）。
Bulk 读失败时 rioutil 会调用 `libusb_reset_device` 重置设备——本库可选择是否实现。

---

## 18. 参考文献

### 18.1 开源实现

- **rioutil** (主要参考): https://github.com/hjelmn/rioutil
  - `include/rioi.h` — opcode 定义、rio_file_t 结构、命令时序注释
  - `librioutil/rio.c` — 设备表、初始化序列
  - `librioutil/rioio.c` — send_command、read_block、write_block、CRC 包构造
  - `librioutil/driver_libusb.c` — libusb 后端（control/bulk 实现）
  - `librioutil/song_management.c` — 上传/下载/删除/覆盖逻辑
  - `librioutil/playlist_file.c` — FIDL/ST10 格式

### 18.2 Linux 内核驱动（gen3，仅参考）

- `drivers/usb/misc/rio500.c` (v4.13 后移除):
  https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/plain/drivers/usb/misc/rio500.c?h=v4.13
- 注意：rio500 是 Rio 500（gen3）驱动，协议与 S-Series **不同**，仅作对比参考。

### 18.3 原厂资料

- 用户提供的 `原版软件/RioUniDrv/RioUniv/riouniv.inf` — VID/PID 表、驱动版本
- `原版软件/rio_soft296/Rio_Software_Ver296.txt` — Rio Music Manager 2.96 发布说明

### 18.4 设备手册

- Rio S50 用户手册: https://www.manualslib.com/manual/314535/Rio-S50.html
  - 第 28 页：MP3/WMA/ID3/Playlist 支持说明

### 18.5 USB 相关

- USB 2.0 规范：Control Transfer、Bulk Transfer、Vendor Request
- libusb 1.0 API: https://libusb.sourceforge.io/api-1.0/

---

## 附录 A：原厂资料交叉验证

本附录记录与原厂 Windows 驱动（`RIOUNIV.SYS` v2.4.0.22）和管理软件
（Rio Music Manager 2.96）的交叉验证结论。所有原厂资料位于仓库
`原版软件/` 目录。

### A.1 `riouniv.inf` 设备 GUID 表

`原版软件/RioUniDrv/RioUniv/riouniv.inf` v2.4.0.22（2003-06-30 发布）中，
每款 Rio 设备的 `RioXxxhw.AddReg` 段都注册了一个 `DeviceGUID`，用于把 USB 设备
暴露给 WMDM (Windows Media Device Manager) Service Provider：

| 机型     | PID     | DeviceGUID                                    | INF 行号 |
|----------|---------|-----------------------------------------------|----------|
| Rio S10  | `0x5005`| `{54413cef-ac05-4ff9-a009-9678eb4ec541}`      | 251      |
| Rio S50  | `0x5006`| `{dc15545f-a470-429b-b1aa-8d45f4168091}`      | 374      |
| Rio S35S | `0x5007`| `{5906da6d-df99-4038-b99f-f2309ea7db94}`      | 333      |
| Rio S30S | `0x5009`| `{d90410c4-bc73-4543-b6f6-0172b477ccbb}`      | 292      |

**交叉验证结论**：INF 中 S-Series 四款机型的 VID/PID 与本规范 §1.1 表完全一致，
证实 `constants.ts` 的 `PRODUCT_RIO_S10/S50/S35S/S30S` 常量正确。

### A.2 WMDM Service Provider CLSID

`riouniv.inf` 末尾的 `[WMDMhw.AddReg]` 段注册了 WMDM Service Provider CLSID：

```inf
[WMDMhw.AddReg]
HKR,,"WMDMSPCLSID",0x0,"{9DA6B0FC-011D-403f-8CBB-0E438BF6BCEA}"
HKR,,"ShowInShell",0x10001,1
```

**意义**：所有 Rio 设备（gen3-gen5）通过同一个 WMDM Service Provider 暴露给
Windows Media Device Manager，再由 Rio Music Manager 通过 WMDM API 访问。
这证实了本规范开头的论断："协议性质：自定义 USB 协议，非 USB Mass Storage"——
Windows 把 Rio 设备识别为 WMDM 设备（便携媒体播放器），而非 Mass Storage 大容量存储。

### A.3 `RIOUNIV.SYS` PE 结构

`原版软件/RioUniDrv/RioUniv/RIOUNIV.sys` 是 Windows 内核驱动（PE32 executable, Intel 80386, native），
大小 16,128 字节。PE 段表：

| 段      | 虚拟地址  | 虚拟大小 | 原始偏移  | 原始大小 |
|---------|-----------|----------|-----------|----------|
| `.text` | `0x00000300` | `0x2b82` | `0x00000300` | `0x2c00` |
| `.rdata`| `0x00002f00` | `0x168`  | `0x00002f00` | `0x180`  |
| `.data` | `0x00003080` | `0x10`   | `0x00003080` | `0x80`   |
| `INIT`  | `0x00003100` | `0x50c`  | `0x00003100` | `0x580`  |
| `.rsrc` | `0x00003680` | `0x450`  | `0x00003680` | `0x480`  |
| `.reloc`| `0x00003b00` | `0x39a`  | `0x00003b00` | `0x400`  |

`INIT` 段是 WDM 驱动特有的，包含 `DriverEntry` 函数，初始化完成后该段可被换出。

### A.4 关键发现：协议魔数全部不在驱动 `.rdata` 段中

用 Python 对 `RIOUNIV.SYS` 全文件扫描以下 14 个协议魔数作为 ASCII 字节序列：

```
CRIODATA, CRIOINFO, CRIOABRT,
SRIORDY, SRIODATA, SRIODONE, SRIONOFL, SRIODELS, SRIODELD, SRIOFMTD,
FIDL, MPG3, PLS , WMDM
```

**扫描结果**：全部 14 个魔数在驱动二进制中**均不存在**。

**结论**：`RIOUNIV.SYS` 是一个**薄壳 USB 管道驱动**——它只负责：
- USB 设备枚举（`DriverEntry` → `AddDevice`）
- 创建命名设备对象供 user-mode 通过 `\\.\RioDevX` 句柄访问
- 在 IOCTL 处理中调用 `UsbBuildSelectInterfaceRequest` /
  `UsbBuildInterruptOrBulkTransferRequest` 把 user-mode 提交的缓冲区转发给 USB 管道

驱动**不包含**任何协议状态机逻辑、CRC32 计算、rio_file_t 序列化。所有协议逻辑
全部位于 user-mode：

- **Rio Music Manager 2.96**（C++/MFC 应用）— 包含 CRIODATA 构造、状态机、
  rio_file_t 结构定义
- 通过 `DeviceIoControl(hDevice, IOCTL_RIO_xxx, inBuf, outBuf)` 把已构造好的字节流
  提交给驱动，驱动只是 `IoCallDriver` 到 USB 栈

**逆向工程意义**：即便用 IDA Pro 反汇编 16KB 的 .sys 文件，也只能确认端点地址、
IOCTL 派发、USB 描述符解析等"传输层"细节；协议层（命令、握手、CRC、结构体）的
权威来源仍是 rioutil 源码（已逆向完成，见 §18.1）。

### A.5 PDB 路径与代码代号

从驱动二进制提取到的 PDB 调试符号路径：

```
S:\marlin_v2_00\empeg\drivers\win32\riouniv\objfre\i386\RIOUNIV.pdb
```

**信息解读**：
- `marlin_v2_00` — Rio 内部固件/软件代号 "Marlin v2.00"
- `empeg` — empeg Ltd 是英国公司，1999 年被 Diamond Multimedia 收购后改名为
  Rio Audio，是 Rio 系列 MP3 播放器的技术研发源头（Rio Car / Central 即 empeg car）
- `drivers\win32\riouniv` — Windows 32 位通用驱动源码目录
- `objfre\i386` — Windows DDK free build（release）输出目录，i386 架构

### A.6 驱动版本与发布日期

`riouniv.inf` 顶部声明：

```inf
[Version]
DriverVer=06/30/2003,2.4.0.22
CatalogFile=RIOUNIV.CAT
```

- **驱动版本**：v2.4.0.22
- **发布日期**：2003-06-30
- **数字签名**：`RIOUNIV.CAT`（8,938 字节，Windows Authenticode 目录文件）
- **支持操作系统**：Windows 98/ME/2000 SP3+/XP SP1+（INF 中 `.Dev.NT.Services`
  段为 NT 内核服务注册，`.Dev` 段为 9x 内核 `*ntkern` 兼容注册）

**Win7 32-bit 兼容性**：用户实测该驱动可在 Windows 7 32-bit 中正常使用
（Windows 7 兼容 Windows XP 驱动模型，32 位 sys 文件可加载）。Win7 64-bit 不兼容
（驱动是 PE32 i386，不是 PE32+ x64）。

### A.7 Rio Music Manager 软件包

`原版软件/rio_soft296/` 包含 Rio Music Manager 2.96 完整安装包：

| 文件                       | 大小       | 说明                                              |
|----------------------------|------------|---------------------------------------------------|
| `Rio_Software_Ver296.txt`  | 2,988 B    | 发布说明（2005-07-25）                              |
| `SetupWrapper.exe`         | 1,474,560 B| 安装器外壳（自解压 → msiexec）                       |
| `setup_files/setup_riomm.exe` | 16,694,033 B | Rio Music Manager 主安装包                       |
| `setup_files/setup_sbupdate.exe` | 4,743,449 B | Rio Internet Updater（固件检查更新）             |

**支持的机型**（摘自 `Rio_Software_Ver296.txt`）：
> Rio Carbon, Ce21xx, Forge, Se510, Nitrus, Cali, Chiba, Fuse, **S10, S50, S30S, S35S**

**版本历史**（v2.96 修复的主要 bug）：
- 修复 RioDJ 无法从已有歌单创建 mix 的问题
- 修复直接编辑设备上歌单时原歌单被覆盖的问题（"Save As..." 进度指示器错误）
- 修复读取包含缺失歌曲的歌单时 Rio Music Manager 崩溃
- 修复 FLAC 文件传输问题

### A.8 原版软件/驱动目录完整结构

项目根目录下的 `原版软件/` 子目录归档了用户提供的 Windows 原版软件和驱动，
可在 Win7 32位 VM 中用于交叉验证（抓包对照、运行原厂 Rio Music Manager）。

```
原版软件/
├─ rio_soft296/                       # Rio Music Manager 2.96 安装包（2005-07-25）
│  ├─ Rio_Software_Ver296.txt         # 2,988 B  发布说明
│  ├─ SetupWrapper.exe                # 1.4 MB   安装器外壳
│  └─ setup_files/
│     ├─ setup_riomm.exe              # 16 MB    Rio Music Manager 主安装包
│     └─ setup_sbupdate.exe           # 4.7 MB   Rio Internet Updater
│
└─ RioUniDrv/RioUniv/                 # 通用 USB 驱动 v2.4.0.22（2003-06-30）
   ├─ readme_98.txt                   # 1.1 KB   Win98 升级说明
   ├─ readme_me.txt                   # 1.2 KB   WinME 升级说明
   ├─ riouniv.cat                     # 8.9 KB   Authenticode 数字签名目录
   ├─ riouniv.inf                     # 36 KB    驱动安装信息（含 16 款机型 VID/PID）
   └─ RIOUNIV.sys                     # 16 KB    PE32 i386 内核驱动
```

**交叉验证结论**：

| 验证项 | 结果 | 对应章节 |
|---|---|---|
| 4 个 S-Series PID（0x5005/06/07/09） | ✅ 与 `constants.ts` 完全一致 | A.1 |
| S50 DeviceGUID | ✅ `{dc15545f-a470-429b-b1aa-8d45f4168091}` | A.1 |
| WMDMSPCLSID | ✅ `{9DA6B0FC-011D-403f-8CBB-0E438BF6BCEA}` | A.2 |
| RIOUNIV.SYS 协议魔数 | ❌ 全部不在 `.rdata` 段（协议逻辑在 user-mode） | A.4 |
| Win7 32-bit 兼容性 | ✅ 用户实测可用 | A.6 |

**Phase 6 真机测试备选方案**：

- **方案 A（首选）**：macOS arm64 真机直测（用户当前环境，Node.js + `usb@2.13.0`）
- **方案 B（备选）**：Win7 32位 VM + Rio Music Manager 2.96 抓包对照
  - 工具：USBPcap + Wireshark
  - 流程：在 Win7 32位 VM 中安装 `原版软件/rio_soft296/SetupWrapper.exe`，
    用 USBPcap 抓取 Rio Music Manager 与设备的 USB 通信，与本实现的时序对照
- **方案 C（深度逆向）**：RIOUNIV.SYS 反汇编
  - 工具：Ghidra / IDA Pro / radare2
  - 目标：提取 USB 控制传输调用模式（`URB_FUNCTION_VENDOR_DEVICE` 等）
  - 注意：A.4 已确认协议魔数不在驱动中，反汇编价值有限，优先级低于方案 A/B
