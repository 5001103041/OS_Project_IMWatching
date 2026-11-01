# TECHNICAL_DOCS.md — IMWatching

> **Hệ thống Web AI Phân tích Hành vi Video và Giám sát Tài nguyên Hệ điều hành Thời gian thực**  
> Tài liệu kỹ thuật chi tiết về kiến trúc phần mềm, cấu trúc dữ liệu, cơ chế điều phối hệ điều hành và danh mục giao diện lập trình ứng dụng (REST API).

---

## 1. Sơ đồ Kiến trúc Tổng thể (System Architecture)

Hệ thống được thiết kế theo kiến trúc phân tầng (Multi-tier Architecture) với sự phân tách rõ ràng giữa giao diện người dùng (Frontend), máy chủ xử lý tác vụ (Backend) và tầng lưu trữ dữ liệu (Database & Media Storage):

```
┌────────────────────────────────────────────────────────────────────────┐
│                        FRONTEND (Svelte 4 + Vite)                      │
│                                                                        │
│  ┌───────────────────────┐  ┌───────────────────────┐  ┌────────────┐  │
│  │ Thêm Video / Quay     │  │ Trình phát Video      │  │ Bảng Thống │  │
│  │ + Đánh giá Cấu hình   │  │ + Dòng thời gian      │  │ kê An ninh │  │
│  │ (UploadAdvisor.svelte)│  │ + Giám sát phần cứng  │  │ (Chart.js) │  │
│  └───────────────────────┘  └───────────────────────┘  └────────────┘  │
│  ┌──────────────────────────────────────────────────┐  ┌────────────┐  │
│  │ Trang Tài nguyên & Hệ điều hành                  │  │ Trợ lý     │  │
│  │ - Chế độ người dùng (Hardware Cards)             │  │ Chatbot    │  │
│  │ - Chế độ kỹ thuật (Canvas 60s, PCB Table, Sync)  │  │ AI (Float) │  │
│  └──────────────────────────────────────────────────┘  └────────────┘  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ HTTP REST API (JWT HS256)
┌───────────────────────────────────▼────────────────────────────────────┐
│                    BACKEND (Rust + Axum + Tokio Runtime)               │
│                                                                        │
│  ┌─────────────────────────────┐      ┌─────────────────────────────┐  │
│  │      Tầng Giao tiếp HTTP    │      │ Module Quản lý HĐH          │  │
│  │   - Axum Routing & Auth     │      │ (os_runtime.rs)             │  │
│  │   - JWT Extractor / Guard   │      │ - Bảng tiến trình con (PCB) │  │
│  │   - Rate Limit & Static Serve│     │ - Cấp phát khóa & Semaphore │  │
│  └──────────────┬──────────────┘      │ - Bộ lập lịch Job Scheduler │  │
│                 │                     └──────────────┬──────────────┘  │
│                 ▼                                    │                 │
│  ┌───────────────────────────────────────────────────▼──────────────┐  │
│  │       Async Worker Pool Pipeline (jobs.rs)                       │  │
│  │  1. Đọc thông tin video (ffprobe qua ProcessGuard)               │  │
│  │  2. Chuyển mã nén H.264 đa luồng (ffmpeg qua ProcessGuard)       │  │
│  │  3. Trích xuất khung hình định kỳ 1 FPS (ffmpeg)                 │  │
│  │  4. Phân tích thị giác AI (ai.rs: Gemini Vision / OpenAI API)    │  │
│  │  5. Lớp phòng vệ chuẩn hóa dữ liệu (Ingestion Guard)             │  │
│  │  6. Ghi nhận thời gian chờ (Waiting Time & Turnaround Time)      │  │
│  └──────────────────────────────────┬───────────────────────────────┘  │
│                                     │                                  │
│  ┌──────────────────────────────────▼───────────────────────────────┐  │
│  │ Module Giám sát Phần cứng (hardware_monitor.rs)                  │  │
│  │ - sysinfo: CPU Usage, RAM, Ổ đĩa phân vùng                       │  │
│  │ - Win32 API: GetSystemTimes, GlobalMemoryStatusEx, GetDiskFree   │  │
│  └──────────────────────────────────┬───────────────────────────────┘  │
└─────────────────────────────────────┼──────────────────────────────────┘
                                      │
              ┌───────────────────────┴───────────────────────┐
              ▼                                               ▼
┌───────────────────────────────┐               ┌───────────────────────────┐
│     Cơ sở Dữ liệu SQLite      │               │     Tệp Lưu trữ Cục bộ    │
│  - users (Tài khoản & Phân quyền)│            │  - media/videos/<video_id>/│
│  - videos (Metadata & Trạng thái)│            │    + upload.webm / input  │
│  - video_frames (Khung hình trích xuất)       │    + final.mp4 (H.264)    │
│  - detected_events (Bằng chứng hành vi)       │    + frame_00001.jpg      │
│  - event_attributes (EAV mở rộng)             │    + frame_00002.jpg      │
│  - settings (Cấu hình Model AI)               │                           │
│  - chat_messages (Lịch sử trao đổi)           │                           │
└───────────────────────────────┘               └───────────────────────────┘
```

---

## 2. Công nghệ Ứng dụng & Cơ sở Lựa chọn

| Phân hệ | Công nghệ | Phiên bản | Lý do kỹ thuật |
|---|---|---|---|
| **Backend Core** | Rust | Stable (1.80+) | Ngôn ngữ hệ thống an toàn bộ nhớ (Memory Safety không cần GC), kiểm soát luồng và tài nguyên phần cứng chính xác, hiệu năng vượt trội cho xử lý media và mạng. |
| **Web Framework** | Axum | 0.7 | Xây dựng trên nền tảng Tokio, tận dụng kiến trúc non-blocking I/O hiện đại, hệ thống extractors an toàn kiểu dữ liệu (Type-safe). |
| **Async Runtime** | Tokio | 1.x | Bộ điều phối luồng bất đồng bộ (Work-stealing thread pool) chuẩn công nghiệp, cung cấp Semaphore, Mutex và kênh giao tiếp Notify cho điều phối HĐH. |
| **Database** | SQLite + SQLx | 3.45+ / 0.8 | Cơ sở dữ liệu file-based nhúng gọn nhẹ, hỗ trợ chế độ WAL (Write-Ahead Logging) cho phép đọc ghi đồng thời cao, truy vấn kiểm tra kiểu tại thời điểm biên dịch (Compile-time verified queries). |
| **Xử lý Video** | FFmpeg & ffprobe | 6.x / 7.x | Công cụ xử lý đa phương tiện hàng đầu thế giới; tích hợp qua đường ống tiến trình con của hệ điều hành với PID thực tế. |
| **Thị giác Máy tính (AI)** | Multimodal Vision API | REST JSON | Tương thích đa nền tảng: Google Gemini (Gemini 3.8 Flash) và OpenAI-compatible gateway (Qwen, MiniMax, DeepSeek). |
| **Frontend Core** | Svelte + TypeScript | 4.2 / 5.x | Biên dịch trực tiếp sang JavaScript tinh gọn không cần Virtual DOM, tốc độ phản hồi cực nhanh, binding dữ liệu trực quan. |
| **Build Tool** | Vite | 5.4 | Hỗ trợ Hot Module Replacement (HMR) tức thời, proxy tích hợp chuyển tiếp API mượt mà, đóng gói tối ưu. |
| **Vẽ biểu đồ** | Chart.js | 4.4 | Thư viện biểu đồ Canvas nhẹ, hiệu năng cao, vẽ biểu đồ dạng tròn, dạng cột và thanh ngưỡng an toàn thời gian thực. |
| **Đồ họa Trực tiếp** | HTML5 Canvas API | Native | Vẽ biểu đồ sóng thời gian thực (60 khung dữ liệu gần nhất) không phụ thuộc thư viện ngoài, tối ưu tài nguyên CPU frontend. |

---

## 3. Thiết kế Module Hệ Điều Hành (OS Runtime Architecture)

Module [`backend/src/os_runtime.rs`](backend/src/os_runtime.rs) hiện thực hóa 5 nguyên lý cốt lõi của môn học Hệ điều hành:

### 3.1. Bảng Khối Điều Khiển Tiến Trình Con (PCB Table) & Chống Tiến Trình Zombie
- **Bảng PCB trong bộ nhớ**: Theo dõi mọi tiến trình con (`ffprobe`, `ffmpeg`) qua cấu trúc `PcbEntry` chứa:
  - `record_id`: Mã định danh bản ghi tuần tự.
  - `os_pid`: Process ID thực tế cấp phát bởi nhân hệ điều hành.
  - `program`: Tên và mục đích thực thi (`ffprobe`, `ffmpeg:transcode`, `ffmpeg:extract_frames`).
  - `state`: Trạng thái vòng đời (`running`, `reaped`, `failed`, `aborted`).
  - `exit_code`: Mã thoát trả về khi tiến trình kết thúc (`exit(0)`).
  - `duration_ms`: Thời gian chiếm dụng CPU thực tế.
- **Triệt tiêu 100% Zombie Process bằng RAII**:
  - Áp dụng mẫu thiết kế **RAII Guard (`ProcessGuard`)**: Khi đối tượng `ProcessGuard` ra khỏi phạm vi thực thi (kể cả trường hợp hàm panic hoặc gặp lỗi bất ngờ), destructor `Drop` tự động kích hoạt để thu hồi tiến trình con (`wait_with_output` / `waitpid`), cập nhật trạng thái PCB sang `Reaped` và trả giấy phép về cho Semaphore.

### 3.2. Đồng Bộ Hóa Đa Cấp & Thứ Tự Khóa Chống Deadlock (Lock Hierarchy)
Hệ thống sử dụng cơ chế cấp phát tài nguyên theo thứ tự tăng dần cố định (**Hierarchical Resource Ordering**) nhằm triệt tiêu điều kiện *Chờ đợi vòng tròn (Circular Wait)*:

```
Level 1: pipeline_semaphore (Giới hạn số tác vụ Worker song song)
    │
    ▼
Level 2: subprocess_semaphore (Giới hạn số tiến trình con OS thực thi cùng lúc)
    │
    ▼
Level 3: critical_section (std::sync::Mutex bảo vệ Bảng PCB & Hàng đợi Job)
```

- Không có bất kỳ luồng nào được phép yêu cầu khóa cấp cao hơn khi đang nắm giữ khóa cấp thấp hơn.
- Nhờ đó, xác suất xảy ra bế tắc (**Deadlock**) hoặc đói tài nguyên (**Starvation**) được chứng minh bằng lý thuyết và kiểm chứng qua bài test stress 20 tác vụ đồng thời đạt 100% an toàn.

### 3.3. Bộ Điều Phối Tác Vụ & Đo Lường Thời Gian (Job Scheduler)
Mô hình **Producer-Consumer Worker Pool** tiếp nhận yêu cầu phân tích video và ghi nhận các chỉ số:
- **Waiting Time**: Thời gian tác vụ nằm trong hàng đợi (*Queued*) trước khi được cấp phát Semaphore để bắt đầu chạy.
- **Turnaround Time**: Tổng thời gian hoàn tất một tác vụ từ khi người dùng bấm xử lý đến khi toàn bộ khung hình được phân tích và thu hồi tài nguyên xong.
- Số liệu trung bình được tính toán tự động và hiển thị thời gian thực lên giao diện kỹ thuật.

### 3.4. Quản Lý Bộ Nhớ An Toàn (Memory Safety & Ring Buffer)
- Các cấu trúc dữ liệu lịch sử tiến trình con (`pcb_history`) và tác vụ (`job_history`) được tổ chức dưới dạng **Ring Buffer (`VecDeque`)** giới hạn cố định tối đa 50 bản ghi.
- Cơ chế này ngăn chặn tuyệt đối hiện tượng rò rỉ bộ nhớ (**Memory Leak**) khi hệ thống hoạt động liên tục trong thời gian dài.

---

## 4. Cơ Sở Dữ Liệu SQLite (Database Schema)

Cơ sở dữ liệu được quản lý qua migration của SQLx (`backend/migrations/`):

### 4.1. Bảng `users` (Quản lý tài khoản)
| Tên cột | Kiểu dữ liệu | Ràng buộc | Diễn giải |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | Định danh người dùng (UUIDv4) |
| `email` | TEXT | UNIQUE, NOT NULL | Địa chỉ email đăng nhập |
| `password_hash` | TEXT | NOT NULL | Mật khẩu băm PBKDF2-HMAC-SHA256 |
| `role` | TEXT | NOT NULL DEFAULT 'member' | Vai trò: `admin` hoặc `member` |
| `created_at` | TEXT | NOT NULL | Thời điểm khởi tạo tài khoản |
| `updated_at` | TEXT | NOT NULL | Thời điểm cập nhật thông tin |

### 4.2. Bảng `videos` (Quản lý tệp video)
| Tên cột | Kiểu dữ liệu | Ràng buộc | Diễn giải |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | Mã định danh video (UUIDv4) |
| `owner_id` | TEXT | FOREIGN KEY → users | Mã người sở hữu |
| `original_filename` | TEXT | NOT NULL | Tên tệp gốc khi tải lên |
| `final_filename` | TEXT | NULL | Tên tệp đã chuẩn hóa H.264 |
| `behavior_category` | TEXT | NULL | Nhãn hành vi chủ đạo |
| `storage_path` | TEXT | NOT NULL | Đường dẫn tương đối lưu tệp |
| `mime_type` | TEXT | NOT NULL | Định dạng mime (`video/mp4`, `video/webm`) |
| `duration_secs` | REAL | NULL | Thời lượng video (giây) |
| `width`, `height` | INTEGER | NULL | Độ phân giải khung hình |
| `fps` | REAL | NULL | Tốc độ khung hình trên giây |
| `file_size_bytes` | INTEGER | NULL | Kích thước tệp (bytes) |
| `status` | TEXT | NOT NULL | `uploaded` → `transcoding` → `extracting` → `analyzing` → `completed` / `failed` |
| `error_message` | TEXT | NULL | Thông điệp lỗi chi tiết nếu thất bại |
| `ai_summary` | TEXT | NULL | Nội dung tóm tắt kết quả phân tích do AI tạo |
| `created_at` | TEXT | NOT NULL | Thời điểm tải lên |

### 4.3. Bảng `detected_events` (Sự kiện và bằng chứng nhận diện)
| Tên cột | Kiểu dữ liệu | Ràng buộc | Diễn giải |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | Mã sự kiện (UUIDv4) |
| `video_id` | TEXT | FOREIGN KEY → videos | Mã video tương ứng |
| `frame_id` | TEXT | NULL | Mã khung hình liên kết |
| `timestamp_secs` | REAL | NOT NULL | Thời điểm bắt đầu sự kiện (dùng để Player tua video) |
| `end_timestamp_secs` | REAL | NULL | Thời điểm kết thúc sự kiện |
| `behavior_name` | TEXT | NOT NULL | Tên hành vi (`walking`, `sitting`, `fighting`...) |
| `severity` | TEXT | NOT NULL | Mức độ: `normal` hoặc `dangerous` |
| `gender` | TEXT | NOT NULL DEFAULT 'unknown' | Giới tính đối tượng (`male`, `female`, `unknown`) |
| `clothing_color` | TEXT | NULL | Màu sắc trang phục chủ đạo |
| `confidence` | REAL | NOT NULL | Độ tin cậy nhận diện (0.0 – 1.0) |
| `ai_reason` | TEXT | NULL | Diễn giải chi tiết lý do từ AI |
| `frame_image_path` | TEXT | NOT NULL | Đường dẫn ảnh bằng chứng trích xuất từ video |
| `subjects_json` | TEXT | NULL | Danh sách đầy đủ các đối tượng cùng thuộc tính |
| `review_status` | TEXT | NOT NULL DEFAULT 'unreviewed' | Trạng thái xét duyệt: `unreviewed`, `confirmed`, `false_positive` |

### 4.4. Bảng `event_attributes` (Thực thể - Thuộc tính - Giá trị / EAV)
| Tên cột | Kiểu dữ liệu | Ràng buộc | Diễn giải |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | Mã bản ghi thuộc tính |
| `event_id` | TEXT | FOREIGN KEY → detected_events | Mã sự kiện liên kết |
| `attribute_type` | TEXT | NOT NULL | Loại thuộc tính (`s0_clothing_type`, `s0_glasses`, `s0_hat`, `s0_age_group`...) |
| `attribute_value` | TEXT | NOT NULL | Giá trị thuộc tính (`t-shirt`, `yes`, `teen`...) |
| `confidence` | REAL | NOT NULL | Độ tin cậy của thuộc tính |

### 4.5. Bảng `settings` (Cấu hình động kênh AI)
| Tên cột | Kiểu dữ liệu | Ràng buộc | Diễn giải |
|---|---|---|---|
| `key` | TEXT | PRIMARY KEY | Khóa cấu hình (`vision_model`, `chat_model`...) |
| `value` | TEXT | NOT NULL | Giá trị cấu hình model đang kích hoạt |
| `updated_at` | TEXT | NOT NULL | Thời điểm thay đổi cấu hình |

---

## 5. Quy Trình AI Phân Tích Hành Vi & Phòng Vệ 4 Lớp

### 5.1. Phân Loại Chuẩn Xác Trang Phục
- **`t-shirt` (Áo thun, áo phông)**: Toàn bộ áo cổ tròn, áo cổ V, áo thun co giãn, áo phông thể thao, áo mặc ở nhà, áo thun đường phố không có hàng cúc cài dọc ngực.
- **`shirt` (Áo sơ mi)**: Chỉ chấp nhận khi nhận diện thấy rõ ràng cổ áo bẻ có cấu trúc và hàng cúc/khuy cài dọc chính giữa ngực.

### 5.2. Cơ Chế Phòng Vệ 4 Lớp Chống Sai Lệch Hành Vi Nguy Hiểm
Nhằm loại trừ tuyệt đối việc AI gán nhãn nhầm các hành vi thường ngày vào danh mục hành vi nguy hiểm, hệ thống triển khai 4 tầng bảo vệ:

1. **Tầng 1 (Prompt Engineering - `ai.rs`)**:
   - Quy định nghiêm ngặt: `walking`, `standing`, `sitting`, `studying`, `talking`, `gesturing`, `phone_use`, `carrying_object`, `drinking`, `eating`, `dancing`, `hugging` BẮT BUỘC mang `severity = "normal"`.
   - Mức `dangerous` bị giới hạn trong: `fighting`, `school_violence`, `assault`, `weapon_use`, `falling`, `no_hands_driving`, `robbery`, `vandalism`.
2. **Tầng 2 (Ingestion Guard Pipeline - `jobs.rs`)**:
   - Trong quá trình nạp kết quả từ AI, hàm `is_inherently_normal` kiểm tra nếu `behavior_name` thuộc danh mục hành vi sinh hoạt thì cưỡng chế ghi `severity = "normal"` trước khi lưu vào database.
3. **Tầng 3 (SQL Aggregator Filter - `handlers/stats.rs`)**:
   - Câu truy vấn SQL đếm sự kiện nguy hiểm trên toàn hệ thống tự động lọc bỏ bất kỳ hành vi thường ngày nào có trong cơ sở dữ liệu.
4. **Tầng 4 (Frontend Presentation Guard - `Dashboard.svelte`)**:
   - Bộ lọc `nonDangerousBehaviors` trên biểu đồ cột màu đỏ ngăn chặn vẽ các hành vi thường ngày, chỉ hiển thị đúng các mối đe dọa an ninh thực sự.

---

## 6. Danh Mục REST API Chi Tiết (API Reference)

Tất cả các endpoint (trừ các endpoint công khai) đều yêu cầu Header xác thực:  
`Authorization: Bearer <JWT_TOKEN>`

### 6.1. Xác thực & Tài khoản (Authentication)
- `POST /api/auth/register`: Đăng ký tài khoản mới (`{ email, password }`).
- `POST /api/auth/login`: Đăng nhập hệ thống, trả về JWT Token và thông tin người dùng.
- `GET /api/auth/me`: Lấy thông tin tài khoản đang đăng nhập.

### 6.2. Quản lý Video & Tải lên (Videos)
- `POST /api/videos`: Tải lên tệp video (Multipart FormData với key `file`).
- `GET /api/videos`: Lấy danh sách video thuộc sở hữu của người dùng hiện tại.
- `GET /api/videos/:id`: Lấy thông tin chi tiết một video (metadata, trạng thái, kích thước).
- `DELETE /api/videos/:id`: Xóa video cùng toàn bộ khung hình và dữ liệu sự kiện liên quan.
- `POST /api/videos/:id/process`: Kích hoạt tiến trình phân tích video chạy nền.
- `GET /api/videos/:id/progress`: Lấy tiến độ xử lý hiện tại (số khung hình đã giải mã / tổng số).

### 6.3. Sự kiện & Dữ liệu Nhận diện (Events)
- `GET /api/events?video_id=&q=&severity=&behavior=&gender=`: Tìm kiếm, lọc và phân trang danh sách sự kiện phát hiện.
- `GET /api/events/:id`: Lấy chi tiết một sự kiện cùng toàn bộ danh sách thuộc tính EAV mở rộng.
- `PATCH /api/events/:id/review`: Đánh giá sự kiện (`confirmed`, `false_positive`).

### 6.4. Trình phát & Phương tiện Truyền thông (Media Streaming)
- `GET /api/media/videos/:id/stream`: Phát video trực tiếp hỗ trợ HTTP 206 Partial Content (tua video mượt mà).
- `GET /api/media/frames/:video_id/:index`: Trả về hình ảnh khung hình JPEG làm bằng chứng nhận diện.

### 6.5. Giám sát Nhân Hệ Điều Hành & Phần Cứng (Telemetry)
- `GET /api/os/metrics`: Trích xuất ảnh chụp trạng thái nhân hệ điều hành thời gian thực:
  - `server_pid`: Process ID của tiến trình máy chủ backend.
  - `current_phase`: Giai đoạn hoạt động hiện tại (Giai đoạn 1: Khởi động, Giai đoạn 2: Xử lý video, Giai đoạn 3: Hoàn tất).
  - `process_table`: Danh sách tiến trình con đang chạy và lịch sử PCB (PID, chương trình, trạng thái, mã thoát, thời gian chạy, số lượng Zombie = 0).
  - `synchronization`: Số lượng Semaphore khả dụng, tổng số giấy phép, số lượt cấp phát Mutex.
  - `scheduler`: Hàng đợi tác vụ đang chờ/đang chạy, thời gian chờ trung bình (*Waiting Time*), thời gian xử lý trung bình (*Turnaround Time*).
- `GET /api/system/hardware`: Đọc thông số phần cứng thực tế từ máy tính (Model CPU, số luồng/nhân, mức tải tức thời, RAM tổng/khả dụng, dung lượng các ổ đĩa, card mạng đang kết nối).

### 6.6. Thống kê & Trợ lý Chatbot (Analytics & AI)
- `GET /api/stats/dashboard`: Báo cáo tổng hợp số lượng video, tỷ lệ an toàn/nguy hiểm và biểu đồ phân bổ hành vi cảnh báo.
- `GET /api/stats/videos/:id`: Thống kê hành vi chi tiết theo một video cụ thể.
- `POST /api/chat/videos/:id`: Hỏi đáp với Chatbot AI về nội dung diễn ra trong video.
- `GET /api/chat/videos/:id/summary`: Tự động tạo bản tóm tắt diễn biến video.
- `GET /api/settings`: Đọc cấu hình các model AI đang áp dụng.
- `PUT /api/settings`: Cập nhật cấu hình model AI cho kênh Vision hoặc Chatbot.
- `GET /api/ai/models`: Dò quét danh sách toàn bộ các model AI khả dụng từ các nhà cung cấp.
