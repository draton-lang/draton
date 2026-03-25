# Draton Design System

Tài liệu này định nghĩa triết lý thiết kế cho toàn bộ hệ sinh thái Draton — logo, website, docs, extension, và mọi thứ visual liên quan. Bất kỳ ai đóng góp thiết kế đều nên đọc và tuân theo.

---

## Triết lý cốt lõi

**Identity, không phải decoration.**

Logo và visual của Draton phải phản ánh đúng bản chất của ngôn ngữ: self-hosted, systems-level, được xây dựng bởi người dùng của chính nó. Mọi quyết định thiết kế đều xuất phát từ câu hỏi: *cái này có nói lên điều gì về Draton không?*

Không làm phức tạp để trông "kỹ thuật". Không thêm chi tiết để trông "đẹp". Nếu một element không mang ý nghĩa, bỏ đi.

---

## Logomark

**Symbol chính:** `@d`

- `@` đại diện cho syntax đặc trưng của Draton (`@type`, `@asm`, các keyword prefix)
- `d` đại diện cho Draton
- Hai ký tự kết hợp tạo thành một "địa chỉ" — đọc ngay ra identity của ngôn ngữ

**Màu:**
- `@` — accent orange `#E05A1A`
- `d` — off-white `#EEEAE0` (trên nền tối) / near-black `#0E0D0B` (trên nền sáng)

**Nền icon (square):** `#0E0D0B`, border-radius `~18%` của chiều rộng

**Không được:**
- Đổi màu `@` sang màu khác
- Thêm drop shadow, glow, gradient vào logomark
- Stretch hoặc distort tỉ lệ
- Dùng font khác cho logomark

---

## Typography

### Font chính: Chakra Petch

| Nguồn | https://fonts.google.com/specimen/Chakra+Petch |
|---|---|
| License | SIL Open Font License 1.1 — free, kể cả commercial |
| Vietnamese | Có hỗ trợ (subset `vietnamese`) |
| Download | https://github.com/google/fonts/tree/main/ofl/chakrapetch |

**Lý do chọn:** Geometric sans-serif với góc cạnh kỹ thuật, thiết kế gốc từ Thái Lan — không generic, không bị nhầm với Inter hay Roboto. Scale tốt từ 16px favicon đến banner lớn.

**Weight usage:**

| Weight | Dùng cho |
|---|---|
| Bold 700 | Logo, display headings, banner |
| SemiBold 600 | Section headings, UI labels, badges |
| Medium 500 | Subheadings, navigation |
| Regular 400 | Body text, docs, descriptions |

### Font code: JetBrains Mono hoặc Fira Code

Chakra Petch không phải monospace — dùng font riêng cho code blocks. Ưu tiên JetBrains Mono (OFL, Vietnamese ok).

---

## Màu sắc

| Tên | Hex | Dùng cho |
|---|---|---|
| Background dark | `#0E0D0B` | Nền icon, dark mode bg |
| Off-white | `#EEEAE0` | Text chính trên nền tối |
| Accent orange | `#E05A1A` | `@` trong logo, CTA, highlight |
| Muted | `#4A4640` | Tagline, secondary text |
| Rule | `#2A2820` | Divider lines |

**Nguyên tắc màu:**
- Accent orange là màu duy nhất được dùng mạnh — không thêm màu accent thứ hai
- Nền tối là default, nền sáng là variant
- Không dùng pure black `#000000` hay pure white `#FFFFFF`

---

## Files

| File | Dùng cho |
|---|---|
| `draton-icon.svg` | VSCode extension, GitHub avatar, favicon source |
| `draton-icon.png` | 512×512, mọi nơi cần raster square icon |
| `draton-readme-banner.svg` | GitHub README header |
| `draton-readme-banner.png` | 1200×400, fallback cho README |

**Khi export file mới:**
- SVG: viewBox chuẩn, font load qua Google Fonts `@import`
- PNG: tối thiểu 512×512 cho icon, 1200×400 cho banner
- Không flatten font thành path trừ khi cần embed offline

---

## Những gì KHÔNG làm

- **Không dùng chainring/gear** — đã bị Rust own
- **Không vẽ symbol abstract không có câu chuyện** — mọi symbol phải giải thích được bằng một câu
- **Không dùng gradient** — flat color only
- **Không thêm icon/illustration phức tạp** — Draton identity đủ mạnh bằng typography
- **Không dùng Inter, Roboto, Arial** cho bất kỳ thứ gì official

---

## Câu hỏi thiết kế

Trước khi thêm bất kỳ visual element nào, hỏi:

1. Cái này nói lên điều gì về Draton?
2. Nếu bỏ đi, logo/design có yếu hơn không?
3. Có ai nhìn vào và nghĩ "à, đây là Rust/Go/generic tech" không?

Nếu câu 1 không có câu trả lời rõ ràng, hoặc câu 3 là "có" — bỏ đi.

---

---

## Áp dụng theo context

### Logo variants
| Variant | File | Dùng khi |
|---|---|---|
| Icon square | `draton-icon.svg / .png` | VSCode extension, GitHub avatar, favicon, app icon |
| README banner | `draton-readme-banner.svg / .png` | GitHub README, docs header |
| Wordmark | `@draton` Chakra Petch Bold 700 | Website header, presentation cover |

### Website / Landing page
- Font: Chakra Petch load qua Google Fonts
- Màu nền default: `#0E0D0B` (dark-first)
- Accent: `#E05A1A` cho CTA button, link hover, highlight
- Heading: Bold 700, letter-spacing nhẹ
- Body: Regular 400, line-height 1.7
- Code blocks: JetBrains Mono hoặc Fira Code

### Docs (VitePress / Starlight / bất kỳ)
- Override font-family sang Chakra Petch
- Code font: JetBrains Mono
- Accent color: `#E05A1A`
- Giữ nền mặc định của framework, chỉ override font và accent

### VSCode Extension
- Icon: `draton-icon.png` 128×128 tối thiểu
- Syntax highlight: dùng `#E05A1A` cho `@` prefix keywords (`@type`, `@asm`...)
- Display name: `Draton` — không thêm suffix

### GitHub
- Repo avatar: `draton-icon.png`
- README: `draton-readme-banner.svg` đặt đầu file
- Social preview (OpenGraph): banner 1200×630, same aesthetic

### Presentations / Slides
- Cover: `@draton` centered, Chakra Petch Bold, nền `#0E0D0B`
- Highlight color: `#E05A1A`
- Không dùng template có sẵn của Google Slides / PowerPoint

### Merchandise / Sticker
- Icon trên nền tối: `@d` cam trắng — canonical version
- Tránh in trên nền trắng

---

*Maintained by Vietrix / Lê Hùng Quang Minh*
