# IMWatching — AI Video Behavior Analysis & OS Monitoring System

Hệ thống Web AI Phân tích Hành vi Video và Giám sát Tài nguyên Hệ điều hành thời gian thực.

Dự án kết hợp giữa **Trí tuệ Nhân tạo Đa phương thức (Multimodal Vision AI)** và **Nguyên lý Hệ điều hành (Operating System Concurrency, Subprocess & Resource Management)**.

---

## 🌟 Tính năng Cốt lõi

### 1. Phân tích Video & Nhận diện Hành vi bằng AI
- **Thêm video linh hoạt**: Hỗ trợ tải tệp video (`.mp4`, `.avi`, `.mov`, `.webm`, `.mkv`) hoặc quay trực tiếp từ webcam/camera.
- **Đánh giá phần cứng trước khi tải lên (Hardware Advisor)**: Tự động đo lường năng lực phần cứng thực tế của máy tính (CPU, RAM, Ổ đĩa, Kết nối mạng) đối chiếu với dung lượng và độ phân giải video để cảnh báo điểm nghẽn và đưa ra khuyến nghị.
- **Pipeline xử lý video đa tiến trình**:
  - `ffprobe`: Trích xuất metadata (độ phân giải, thời lượng, FPS).
  - `ffmpeg`: Chuyển mã sang chuẩn H.264/MP4 và trích xuất khung hình định kỳ (1 FPS).
  - **Vision AI Engine**: Gửi khung hình đến mô hình Vision (Gemini 3.8 / Qwen / MiniMax...) để nhận diện hành vi, giới tính, màu áo, loại trang phục (áo thun vs áo sơ mi), phụ kiện (kính, mũ) và mức độ an toàn (`normal` vs `dangerous`).
- **Trình phát video thông minh**: Đồng bộ dòng thời gian (Timeline) với từng sự kiện phát hiện; bấm vào sự kiện để tua ngay đến khung hình tương ứng; tìm kiếm sự kiện theo từ khóa; tự động tạo bản tóm tắt kết quả phân tích.
- **Trợ lý Chatbot chuyên sâu**: Hỏi đáp trực tiếp về diễn biến, số lượng vụ việc và các đối tượng xuất hiện trong video.

### 2. Giám sát Tài nguyên & Nhân Hệ điều hành (OS Runtime Telemetry)
- **Bảng điều khiển tiến trình con (PCB - Process Control Block)**: Theo dõi trực tiếp PID thực tế của các tiến trình con (`ffmpeg`, `ffprobe`), trạng thái hoạt động và thời gian thực thi.
- **Triệt tiêu 100% Tiến trình Zombie**: Quản lý vòng đời tiến trình bằng RAII Guard (`ProcessGuard`) kết hợp cơ chế `wait_with_output` thu hồi tài nguyên sạch sẽ ngay khi tiến trình kết thúc.
- **Đồng bộ hóa & Kiểm soát Miền găng (Synchronization & Critical Section)**:
  - `tokio::sync::Semaphore`: Giới hạn tải pipeline và số lượng tiến trình con song song.
  - `std::sync::Mutex`: Bảo vệ dữ liệu dùng chung trong vùng nhớ miền găng.
  - `Thứ tự cấp phát tài nguyên cố định (Lock Hierarchy)`: `Level 1: pipeline_semaphore`, `Level 2: subprocess_semaphore`, `Level 3: critical_section (Mutex)` — triệt tiêu hoàn toàn nguy cơ Deadlock.
- **Bộ điều phối tác vụ (Scheduler)**: Tính toán chính xác thời gian chờ trong hàng đợi (*Waiting Time*) và tổng thời gian hoàn tất (*Turnaround Time*).
- **Hai chế độ giám sát chuyên biệt**:
  - **Chế độ người dùng**: Giao diện trực quan hiển thị thông số phần cứng thực tế (CPU, RAM, Ổ đĩa, Kết nối mạng) và đánh giá khả năng xử lý của máy tính.
  - **Chế độ kỹ thuật**: Biểu đồ sóng tài nguyên thời gian thực (60 giây gần nhất), danh sách tiến trình con PCB, cơ chế đồng bộ, hàng đợi tác vụ và các API/hàm hệ thống đang gọi (`CreateProcessW`, `waitpid`, `GetSystemTimes`, `GlobalMemoryStatusEx`, `GetDiskFreeSpaceExW`).
- **Giám sát trực tiếp trên màn hình xem video**: Thanh đo tải CPU, RAM, tiến trình con và Semaphore khả dụng ngay dưới trình phát video.

---

## 🏗 Kiến trúc Hệ thống

```
┌────────────────────────────────────────────────────────┐
│                   FRONTEND (Svelte 4 + Vite)           │
│  - Điều hướng Sidebar thu gọn                          │
│  - Thêm Video & Đánh giá cấu hình phần cứng            │
│  - Trình phát video + Timeline + Giám sát trực tiếp    │
│  - Trang Tài nguyên và Hệ điều hành (2 Chế độ)         │
│  - Thống kê an ninh (Dashboard) & Trợ lý Chatbot       │
└───────────────────────────┬────────────────────────────┘
                            │ HTTP REST API (JWT HS256)
┌───────────────────────────▼────────────────────────────┐
│                BACKEND (Rust + Axum + Tokio)           │
│  ┌───────────────────────┐  ┌───────────────────────┐  │
│  │    Web & Auth API     │  │   OS Runtime Manager  │  │
│  │  Axum Routes + State  │  │  PCB Table + Semaphores│  │
│  └───────────┬───────────┘  └───────────┬───────────┘  │
│              │                          │              │
│  ┌───────────▼──────────────────────────▼───────────┐  │
│  │       Async Worker Pool Pipeline (jobs.rs)       │  │
│  │   1. ffprobe (metadata)                          │  │
│  │   2. ffmpeg transcode (.mp4 H.264)               │  │
│  │   3. ffmpeg extract frames (1 FPS JPEG)          │  │
│  │   4. AI Vision Client (Gemini / CommandCode)     │  │
│  │   5. Ingestion Guard (Phòng vệ 4 lớp Normal)     │  │
│  └───────────────────────┬──────────────────────────┘  │
└──────────────────────────┼─────────────────────────────┘
                           │
           ┌───────────────┴───────────────┐
           ▼                               ▼
┌───────────────────────┐     ┌────────────────────────┐
│ SQLite (imwatching.db)│     │  Tệp Video & Khung hình│
│  - users, videos      │     │  - media/videos/<id>/  │
│  - detected_events    │     │  - *.mp4, frame_*.jpg  │
│  - event_attributes   │     └────────────────────────┘
│  - settings, chat     │
└───────────────────────┘
```

---

## 🚀 Hướng dẫn Cài đặt & Chạy Hệ thống

### Cách 1: Khởi động Nhanh bằng 1 Cú Nhấp Chuột (Khuyến nghị cho Windows)

Người dùng hoặc người khác sau khi tải dự án về chỉ cần:
- **Nhấp đúp chuột vào file `START.bat` (hoặc `KHOI_DONG.bat`)** ở thư mục gốc.

File script sẽ tự động thực hiện từ A đến Z:
1. Kiểm tra môi trường (Rust, Node.js, FFmpeg).
2. Tự động khởi tạo file cấu hình `.env` cho Backend nếu chưa có.
3. Tự động chạy `npm install` cài đặt thư viện Frontend nếu máy mới tải về chưa có `node_modules`.
4. Mở song song 2 cửa sổ chạy **Backend Server (port 8080)** và **Frontend Web (port 5173)**.
5. Tự động mở trình duyệt web tại `http://localhost:5173`.

> Khi muốn dừng hệ thống, chỉ cần nhấp đúp file **`STOP.bat`** (hoặc `DUNG_HE_THONG.bat`).

---

### Cách 2: Khởi động Thủ công (2 Terminal)

#### Yêu cầu Tiên quyết
- **Rust**: Phiên bản stable mới nhất (`rustc`, `cargo`) — [https://rustup.rs](https://rustup.rs)
- **Node.js**: Phiên bản 20+ và `npm`
- **FFmpeg & ffprobe**: Đã cài đặt và có trong biến môi trường `PATH` (kiểm tra bằng lệnh `ffmpeg -version` và `ffprobe -version`)

#### Bước 1: Khởi động Backend (Terminal 1)

```powershell
cd backend
cargo run
```

Máy chủ API sẽ khởi động tại: `http://localhost:8080`  
Kiểm tra sức khỏe hệ thống: `http://localhost:8080/api/health`

> **Lưu ý:** Khi khởi động, Backend sẽ tự động chạy migration database, khởi tạo file SQLite tại `backend/data/imwatching.db` và tạo thư mục lưu trữ `backend/media/`.

#### Bước 2: Khởi động Frontend (Terminal 2)

```powershell
cd frontend
npm install   # chỉ cần chạy lần đầu
npm run dev
```

Mở trình duyệt tại: **`http://localhost:5173`**

> Vite đã được cấu hình proxy tự động chuyển tiếp toàn bộ các yêu cầu `/api` sang `http://localhost:8080`.

---

## 📊 Script Đo lường Hiệu năng CPU qua 3 Giai đoạn (HĐH)

Hệ thống cung cấp script PowerShell tự động ghi nhận và đo lường tải tài nguyên qua 3 giai đoạn hoạt động của Hệ điều hành:
- **Giai đoạn 1**: Khởi động hệ thống (Trạng thái nghỉ ban đầu, CPU < 1%).
- **Giai đoạn 2**: Đang xử lý video (Tiến trình con ffmpeg/ffprobe hoạt động, tải CPU cao).
- **Giai đoạn 3**: Hoàn tất xử lý hậu kỳ (Thu hồi toàn bộ tiến trình con, 0 tiến trình Zombie).

### Cách thực hiện benchmark:
Mở một Terminal PowerShell mới và chạy:
```powershell
cd D:\CNPM\Hey_RUworking\IMWatching
.\scripts\benchmark_cpu_phases.ps1
```
* Script sẽ tự động lấy mẫu CPU, RAM, số lượng luồng (Threads) và tiến trình con mỗi 1 giây.
* Sau khi bật script, hãy vào trang web tải lên một video và bấm **Xác nhận tải lên và phân tích ngay**.
* Kết quả báo cáo tổng kết chi tiết sẽ hiển thị trực quan trên console và tự động xuất ra file **`benchmark_cpu_results.csv`**.

---

## ⚙️ Cấu hình Kênh AI Đa Dạng (Vision & Chat)

Hệ thống cho phép cấu hình độc lập 2 kênh AI trực tiếp từ giao diện trang **Cài đặt** (lưu trữ trong SQLite, không cần khởi động lại máy chủ):

| Kênh AI | Mục đích sử dụng | Yêu cầu kỹ thuật | Model khuyến nghị |
|---|---|---|---|
| **Vision Channel** | Phân tích từng khung hình video | Bắt buộc mô hình đa phương thức (nhận ảnh) | `gemini-3.8-flash`, `Qwen/Qwen3.7-Flash`, `MiniMaxAI/MiniMax-M3` |
| **Chatbot Channel** | Hỏi đáp và tóm tắt sự kiện | Chỉ cần mô hình xử lý văn bản (tiết kiệm chi phí) | `deepseek/deepseek-v4-flash`, `gemini-3.8-flash` |

Trang Cài đặt tự động gọi API `GET /api/ai/models` để quét toàn bộ các model đang hoạt động từ Google Gemini API và CommandCode Aggregator.

---

## 🛡️ Cơ chế Phòng vệ Đa tầng Nhận diện Hành vi & Trang phục

1. **Phân biệt chuẩn xác Áo sơ mi vs Áo thun**:
   - `t-shirt` (áo thun, áo phông): Mọi loại áo cổ tròn, áo thun co giãn, áo thể thao, áo mặc thường ngày hoặc ở nhà.
   - `shirt` (áo sơ mi): Bắt buộc phải có cổ áo bẻ có cấu trúc và hàng cúc/khuy cài dọc chính giữa ngực.
2. **Quy tắc An toàn Nghiêm ngặt**:
   - Các hành vi sinh hoạt bình thường: `walking` (đi bộ), `gesturing` (ra hiệu), `talking` (nói chuyện), `standing` (đứng), `sitting` (ngồi), `studying` (học tập)... **tuyệt đối không bao giờ bị đánh dấu là nguy hiểm**.
   - Mức độ `dangerous` chỉ dành riêng cho các nguy cơ an ninh thực tế: `fighting` (đánh nhau), `school_violence` (bạo lực học đường), `assault` (hành hung), `weapon_use` (dùng vũ khí), `falling` (té ngã/tai nạn), `no_hands_driving` (lái xe buông tay), `robbery` (cướp giật).
   - Thiết lập **phòng vệ 4 lớp**: Prompt Engine → Pipeline Ingestion Guard → SQL Stats Aggregator Filter → Frontend Chart Filter.

---

## 📖 Tài liệu Kỹ thuật Tham khảo

- [`TECHNICAL_DOCS.md`](TECHNICAL_DOCS.md): Tài liệu kiến trúc phần mềm, cấu trúc dữ liệu, cơ sở dữ liệu SQLite và danh mục REST API đầy đủ.
- [`OS_APPLICATION_REPORT.md`](OS_APPLICATION_REPORT.md): Báo cáo ứng dụng môn học Hệ điều hành (Chi tiết quản lý tiến trình, luồng, điều phối, tránh Deadlock và kiểm thử áp lực).
