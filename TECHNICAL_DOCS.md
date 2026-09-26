# TECHNICAL_DOCS.md — IMWatching

> Hệ thống Web AI Phân tích Hành vi và Trích xuất Dữ liệu Video (đồ án môn học)

Tài liệu này mô tả kiến trúc, luồng dữ liệu, stack công nghệ, schema database và hướng dẫn chạy local cho dự án **IMWatching**.

---

## 1. Sơ đồ Kiến trúc Hệ thống (System Architecture)

```
┌──────────────────────────────┐         ┌──────────────────────────────────────────────┐
│        FRONTEND (Svelte)      │         │            BACKEND (Rust + Axum)             │
│                               │         │                                              │
│  ┌─────────────────────────┐  │  HTTP   │  ┌────────────────────────────────────────┐  │
│  │ Recorder.svelte         │  │ ──────► │  │ POST /api/videos (upload)               │  │
│  │  getUserMedia +         │  │ upload  │  │  → save file (media/videos/<id>/)       │  │
│  │  MediaRecorder → .webm  │  │ .webm   │  │  → INSERT videos (status=uploaded)     │  │
│  └─────────────────────────┘  │         │  │  → return video_id                      │  │
│                               │         │  └────────────────────────────────────────┘  │
│  ┌─────────────────────────┐  │         │                                              │
│  │ VideoPlayer.svelte      │  │ trigger │  ┌────────────────────────────────────────┐  │
│  │  custom <video> player  │ ────────► │  │ POST /api/videos/:id/process            │  │
│  │  + timeline + seek       │  │         │  │  → spawn background pipeline (jobs.rs) │  │
│  └─────────────────────────┘  │         │  └────────────────┬───────────────────────┘  │
│                               │         │                   │                          │
│  ┌─────────────────────────┐  │  poll   │  ┌────────────────▼───────────────────────┐  │
│  │ Dashboard.svelte        │ ◄──────── │  │  BACKGROUND PIPELINE (jobs.rs)          │  │
│  │  Chart.js doughnut      │  │ stats  │  │                                          │  │
│  └─────────────────────────┘  │         │  │  1. ffprobe → duration/dim/fps          │  │
│                               │         │  │  2. ffmpeg transcode → .mp4             │  │
│  ┌─────────────────────────┐  │ search  │  │  3. ffmpeg -vf fps=1 → frame_NNNN.jpg  │  │
│  │ Search Bar              │ ────────► │  │  4. loop frames:                        │  │
│  └─────────────────────────┘  │         │  │       ai.rs::analyze_frame(jpeg)        │  │
│                               │         │  │       → Vision API (OpenAI-compatible)  │  │
│  ┌─────────────────────────┐  │  chat   │  │       → INSERT detected_events          │  │
│  │ Floating Chatbot        │ ────────► │  │  5. rename final file + mark completed  │  │
│  └─────────────────────────┘  │         │  └────────────────────────────────────────┘  │
└──────────────────────────────┘         │                                                │
                                          │  ┌────────────────────────────────────────┐  │
                                          │  │ SQLite (sqlx)                           │  │
                                          │  │  users · videos · video_frames         │  │
                                          │  │  detected_events · event_attributes   │  │
                                          │  └────────────────────────────────────────┘  │
                                          └──────────────────────────────────────────────┘
```

### Luồng dữ liệu chính

1. **Thu thập**: Người dùng mở trang **Recorder** → browser gọi `getUserMedia` + `MediaRecorder` quay webcam → tạo `.webm`.
2. **Upload**: Frontend `POST /api/videos` (multipart) → backend lưu file vào `media/videos/<id>/`, tạo row `videos` (status=`uploaded`).
3. **Xử lý (trigger)**: Frontend gọi `POST /api/videos/:id/process` → backend spawn background pipeline.
4. **Pipeline** (chạy nền trong `jobs.rs`):
   - `ffprobe` lấy metadata (duration, resolution, fps).
   - `ffmpeg` transcode `.webm` → `.mp4` (H.264, nếu cần).
   - `ffmpeg -vf fps=1` cắt frame JPEG (`frame_00001.jpg`, …).
   - Mỗi frame gửi base64 cho AI Vision (`ai.rs`) → nhận JSON `{behaviors: [{behavior_name, severity, gender, clothing_color, ...}]}`.
   - Mỗi hành vi phát hiện → `INSERT detected_events` + lưu `frame_image_path` làm bằng chứng.
5. **Hiển thị**: Frontend poll `GET /api/videos/:id` (status) + `GET /api/events?video_id=` (danh sách sự kiện) → vẽ timeline + player seek.
6. **Chatbot**: `POST /api/chat/videos/:id` → backend query events của video đó → build context JSON → AI trả lời.

---

## 2. Công nghệ ứng dụng

| Lớp | Công nghệ | Lý do chọn |
|---|---|---|
| **Backend** | Rust + Axum 0.7 | Hiệu năng cao, an toàn bộ nhớ; Axum là web framework async hiện đại, dùng extractors. Kế thừa từ RUworking. |
| **Database** | SQLite + SQLx | Không cần server DB riêng, file-based, đủ cho đồ án; SQLx compile-time-checked queries + migrate. |
| **Auth** | JWT (HS256) + PBKDF2-HMAC-SHA256 | Không cần Google OAuth cho đồ án; PBKDF2 (tự triển khai bằng `hmac`+`sha2`) tránh thêm dependency nặng (argon2). |
| **Video processing** | FFmpeg/ffprobe (qua `std::process::Command`) | Đơn giản, dễ debug, không đau build native binding; FFmpeg là chuẩn công nghiệp. |
| **AI Vision** | OpenAI-compatible `POST /chat/completions` | Tương thích OpenAI, Azure, Gemini-compatible gateway, Ollama, vLLM, LM Studio — linh hoạt đổi model. |
| **Frontend** | Svelte 4 + Vite 5 | Nhẹ, không virtual DOM, code trực quan; Vite build nhanh. |
| **Charts** | Chart.js | Nhẹ, API rõ ràng, tích hợp tốt với Svelte (Recharts chỉ cho React). |
| **Camera** | WebRTC (`getUserMedia`) + `MediaRecorder` API | Native browser, không cần thư viện; ghi `.webm` (VP9/VP8). |
| **Media serving** | Axum + HTTP Range | Cho phép video player seek mượt (browser yêu cầu Range support). |

---

## 3. Database Schema

Chi tiết file migration nằm trong `backend/migrations/`. Tóm tắt:

### `users`
| Cột | Kiểu | Ghi chú |
|---|---|---|
| `id` | TEXT PK | UUID |
| `email` | TEXT UNIQUE | lowercase |
| `name` | TEXT | tuỳ chọn |
| `password_hash` | TEXT | `pbkdf2_sha256$<iters>$<salt_b64>$<hash_b64>` |
| `role` | TEXT | `admin` \| `member` |
| `created_at`, `updated_at` | TEXT | UTC ISO |

### `videos`
| Cột | Kiểu | Ghi chú |
|---|---|---|
| `id` | TEXT PK | UUID |
| `owner_id` | TEXT FK→users | |
| `original_filename` | TEXT | tên file browser gửi (thường `.webm`) |
| `final_filename` | TEXT | `<Category>_<HHMMSS_DDMMYYYY>.mp4` |
| `behavior_category` | TEXT | hành vi chính (đặt tên file) |
| `storage_path` | TEXT | đường dẫn tương đối |
| `mime_type`, `duration_secs`, `width`, `height`, `fps`, `file_size_bytes` | | metadata |
| `status` | TEXT | `uploaded`→`transcoding`→`extracting`→`analyzing`→`completed`/`failed` |
| `error_message` | TEXT | nếu failed |

### `video_frames`
| Cột | Kiểu | Ghi chú |
|---|---|---|
| `id` | TEXT PK | |
| `video_id` | TEXT FK→videos | |
| `frame_index` | INTEGER | 1-based (khớp `frame_%05d.jpg`) |
| `timestamp_secs` | REAL | giây trong video (để seek) |
| `storage_path` | TEXT | đường dẫn frame |
| `analyzed` | INTEGER | 0/1 |

### `detected_events` (bằng chứng — lõi nghiệp vụ)
| Cột | Kiểu | Ghi chú |
|---|---|---|
| `id` | TEXT PK | |
| `video_id` | TEXT FK→videos | |
| `frame_id` | TEXT FK→video_frames | |
| `timestamp_secs` | REAL | **dùng để player seek** |
| `behavior_name` | TEXT | `walking`, `fighting`, `no_hands_driving`… |
| `severity` | TEXT | `normal` \| `dangerous` |
| `gender` | TEXT | `male` \| `female` \| `unknown` (denormalized để tìm nhanh) |
| `clothing_color` | TEXT | `red`, `white`… (denormalized để tìm nhanh) |
| `confidence` | REAL | 0.0–1.0 |
| `ai_reason` | TEXT | lý do thô từ AI |
| `frame_image_path` | TEXT | ảnh bằng chứng (backend điền, **không** do AI trả) |
| `review_status` | TEXT | `unreviewed` \| `confirmed` \| `false_positive` |

### `event_attributes` (EAV mở rộng)
| Cột | Kiểu | Ghi chú |
|---|---|---|
| `event_id` | TEXT FK→detected_events | |
| `attribute_type` | TEXT | `subject_2_gender`, `glasses`, `object`… |
| `attribute_value` | TEXT | |
| `confidence` | REAL | |

### `chat_messages` (tuỳ chọn, lịch sử chatbot)
| Cột | Kiểu | Ghi chú |
|---|---|---|
| `user_id`, `video_id` | TEXT FK | |
| `role` | TEXT | `user` \| `assistant` |
| `content` | TEXT | |

> **Vì sao denormalize `gender` + `clothing_color` vào `detected_events`?** Đây là 2 thuộc tính *bắt buộc* theo spec, dùng trong search rất thường xuyên. Đưa thành cột riêng + index giúp query "áo đỏ" / "nữ" nhanh hơn EAV JOIN. Các thuộc tính phụ (subject thứ 2+, kính, vật thể…) lưu EAV.

---

## 4. Prompt Engineering (Vision AI)

System prompt định nghĩa output JSON chặt chẽ (xem `backend/src/ai.rs`):

```text
You are a video-frame behavior analyzer for a surveillance/education project.

You receive ONE image (a single video frame). Analyze it and return ONLY a
valid JSON object, no markdown, no prose. The object must have EXACTLY this shape:

{
  "behaviors": [
    {
      "behavior_name": "walking|standing|studying|fighting|school_violence|no_hands_driving|...",
      "severity": "normal|dangerous",
      "confidence": 0.0,
      "reason": "short explanation in Vietnamese",
      "subjects": [
        {
          "gender": "male|female|unknown",
          "clothing_color": "a main color word (red, white, black, ...) or unknown",
          "confidence": 0.0
        }
      ]
    }
  ]
}
```

**Quy ước:**
- `frame_image_path` **không** do AI trả về — AI không biết đường dẫn file. Backend gán sau khi lưu frame.
- `behavior_name`: lowercase_snake_case từ danh sách cố định.
- `severity`: `dangerous` cho đánh nhau, bạo lực học đường, lái xe bỏ 2 tay…
- Mỗi subject **bắt buộc** có `gender` và `clothing_color`.
- Parse dùng `parse_verdict_json`: tìm `{...}` đầu tiên (chịu markdown fence/prose).

---

## 5. Hướng dẫn chạy (Run guide)

### Yêu cầu môi trường
- **Rust** stable (`cargo`, `rustc`) — <https://rustup.rs>
- **Node.js** 20+ (`node`, `npm`)
- **FFmpeg** + **ffprobe** trên PATH (hoặc cấu hình `FFMPEG_BIN`/`FFPROBE_BIN`)

Kiểm tra:
```powershell
cargo --version
node --version
ffmpeg -version
ffprobe -version
```

### Backend
```powershell
cd D:\CNPM\Hey_RUworking\IMWatching\backend
# Tạo .env từ .env.example, đặt JWT_SECRET (>= 32 ký tự)
# Copy-Item .env.example .env
cargo run
# → http://localhost:8080/api/health
```

`.env` tối thiểu:
```env
PORT=8080
DATABASE_URL=sqlite://./data/imwatching.db?mode=rwc
JWT_SECRET=<random-32-chars>
MEDIA_DIR=./media
# AI (tuỳ chọn)
AI_ENABLED=false
AI_BASE_URL=
AI_API_KEY=
AI_MODEL=gpt-4o
# FFmpeg
FFMPEG_BIN=ffmpeg
FFPROBE_BIN=ffprobe
FRAME_FPS=1
```

> Migrations tự chạy khi khởi động. Để admin hoá 1 email, thêm vào `ADMIN_EMAILS=you@example.com` **trước khi đăng ký**.

### Frontend
```powershell
cd D:\CNPM\Hey_RUworking\IMWatching\frontend
npm install
npm run dev
# → http://localhost:5173
```

Vite proxy `/api` → `http://localhost:8080` (xem `vite.config.ts`).

### Luồng demo
1. Mở <http://localhost:5173> → Đăng ký tài khoản.
2. Vào **🎥 Quay video** → cấp quyền camera → quay → **Tải lên & phân tích**.
3. Backend lưu file + spawn pipeline (FFmpeg + AI). Xong thì tự chuyển sang trang player.
4. **VideoPlayer**: xem video, click sự kiện bên cạnh → seek đúng timestamp; tìm kiếm "áo đỏ".
5. **Dashboard**: biểu đồ nguy hiểm vs bình thường.
6. **Chatbot** (cần bật AI_ENABLED): hỏi "Có bao nhiêu vụ đánh nhau?".

---

## 6. API Reference

| Method | Path | Auth | Mục đích |
|---|---|---|---|
| `GET` | `/api/health` | — | Health check |
| `POST` | `/api/auth/register` | — | Đăng ký (email/password) → trả JWT |
| `POST` | `/api/auth/login` | — | Đăng nhập → trả JWT |
| `GET` | `/api/auth/me` | Bearer | Thông tin user hiện tại |
| `POST` | `/api/videos` | Bearer | Upload video (multipart `file`) |
| `GET` | `/api/videos` | Bearer | Danh sách video của user |
| `GET` | `/api/videos/:id` | Bearer | Chi tiết 1 video (status, metadata) |
| `POST` | `/api/videos/:id/process` | Bearer | Kích hoạt pipeline phân tích |
| `GET` | `/api/events?video_id=&q=&severity=&behavior=&gender=` | Bearer | List/search/filter events |
| `GET` | `/api/events/:id` | Bearer | Chi tiết 1 event + attributes |
| `GET` | `/api/stats/overview` | Bearer | Tổng nguy hiểm vs bình thường + by_behavior |
| `GET` | `/api/stats/videos/:id` | Bearer | Thống kê theo video |
| `POST` | `/api/chat/videos/:id` | Bearer | Hỏi chatbot về video |
| `GET` | `/api/media/videos/:id/stream` | Bearer | Stream video (cho player) |
| `GET` | `/api/media/frames/:video_id/:index` | Bearer | Ảnh frame bằng chứng |

`Bearer` = JWT HS256, TTL 24h. Claims: `{sub, role, iat, exp, jti}`.
