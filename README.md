# IMWatching

Hệ thống Web AI phân tích hành vi và trích xuất dữ liệu từ video (đồ án môn học).

## Thành phần
- `backend/` — Rust + Axum + SQLite (REST API, auth JWT, upload video, FFmpeg, AI Vision Gemini).
- `frontend/` — Svelte + Vite (SPA): quay camera, video player + timeline, biểu đồ, tìm kiếm, chatbot.

---

## 🚀 Chạy local (2 terminal)

### Terminal 1 — Backend

```powershell
cd D:\CNPM\Hey_RUworking\IMWatching\backend

# 1. Chuẩn bị .env (đã có sẵn, chỉnh JWT_SECRET + AI nếu cần)
#    .env đã bật AI với Gemini. Nếu muốn tắt AI: đặt AI_ENABLED=false

# 2. Chạy
cargo run
# → http://localhost:8080/api/health
```

> Backend tự chạy migrations + tạo file `data/imwatching.db` và thư mục `media/`.

### Terminal 2 — Frontend

```powershell
cd D:\CNPM\Hey_RUworking\IMWatching\frontend

# 1. Cài dependency (lần đầu)
npm install

# 2. Chạy dev server
npm run dev
# → http://localhost:5173
```

> Vite tự proxy `/api` → `http://localhost:8080` (xem `vite.config.ts`), nên không cần cấu hình gì thêm.

---

## 🎬 Luồng sử dụng

1. Mở <http://localhost:5173> → **Đăng ký** tài khoản (email/password bất kỳ).
2. Vào **🎥 Quay video** → cho phép camera → bấm **Bắt đầu quay** → **Dừng quay** → **Tải lên & phân tích**.
3. App tự chuyển sang trang **VideoPlayer**:
   - Xem video, đợi trạng thái `Đang cắt frame` → `Đang phân tích AI` → `Hoàn tất`.
   - Bên cạnh hiện danh sách sự kiện (hành vi + giới tính + màu áo + thumbnail).
   - **Click 1 sự kiện** → video tự seek đúng timestamp.
   - **Ô tìm kiếm**: gõ "đánh nhau", "áo đỏ", "nữ" để lọc.
4. **📊 Thống kê** — biểu đồ nguy hiểm vs bình thường.
5. **💬 Chatbot** (nút tròn góc phải dưới) — hỏi "Có bao nhiêu vụ đánh nhau?".

---

## 📦 Build production (tuỳ chọn)

```powershell
cd frontend
npm run build       # → frontend/dist/
npm run preview     # xem thử bản build
```

---

## ⚙️ Cấu hình AI — 2 kênh, đổi model trên giao diện

Vào **⚙️ Cài đặt** trên web để chọn model, không cần sửa `.env`. Lựa chọn lưu vào
SQLite và có hiệu lực ngay.

| Kênh | Dùng cho | Yêu cầu model |
|---|---|---|
| **👁 Vision** | Phân tích frame (hình ảnh) | **Bắt buộc đa phương thức (nhận ảnh)** |
| **💬 Chat** | Chatbot (chỉ text JSON) | Model text là đủ → dùng loại rẻ |

### Provider trong `backend/.env`

```env
# Gemini (Google)
AI_ENABLED=true
AI_API_KEY=<gemini-key>

# CommandCode (aggregator trả phí — nhiều model rẻ)
COMMANDCODE_BASE_URL=https://api.commandcode.ai/provider/v1
COMMANDCODE_API_KEY=<commandcode-key>
```

Trang Cài đặt tự gọi `GET /api/ai/models` để **dò toàn bộ model còn sống** từ cả
hai provider (đã dò được 61 model Gemini + 81 model CommandCode), nên bạn chọn được
cả những model không nằm trong danh sách đề xuất.

### ✅ Model vision đã kiểm chứng (trên frame thật)

| Model | Kết quả | Dùng được |
|---|---|---|
| `gemini-3.8-flash` / `gemini-3.5-flash-lite` | standing / female / black | ✅ |
| `Qwen/Qwen3.7-Flash` | standing / female / black | ✅ |
| `MiniMaxAI/MiniMax-M3` | standing / female / black | ✅ |
| `Qwen/Qwen3.8-Omni-Flash` | standing / female / black | ✅ |
| `deepseek/deepseek-v4-flash-vision-exp` | standing / female / black | ✅ |
| `xiaomi/mimo-v2.5` | response rỗng | ❌ **text-only** |

> ⚠️ **MiMo V2.5 không dùng được cho vision** (trả về rỗng khi gửi ảnh). Nó vẫn
> chọn được ở kênh **chat**.

### 💰 Tiết kiệm chi phí

Chatbot chỉ gửi JSON text → đặt `chat_model = deepseek/deepseek-v4-flash` (rẻ nhất).
Vision cần model nhận ảnh → dùng Gemini free hoặc `Qwen/Qwen3.7-Flash`.

---

## 📚 Tài liệu

Xem `TECHNICAL_DOCS.md` để biết kiến trúc, schema và API reference đầy đủ.
