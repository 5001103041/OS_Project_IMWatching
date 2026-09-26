/**
 * Shared formatting and localization utilities for IMWatching.
 */

/**
 * Cleanly renders markdown text into safe, stylized HTML.
 * Handles bold, italics, bullets, section titles, and eliminates raw asterisks.
 */
export function renderMarkdown(raw: string | null | undefined): string {
  if (!raw) return '';

  // 1. Escape HTML entities to prevent injection
  let text = raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  // 2. Bold italic: ***text*** -> <strong><em>text</em></strong>
  text = text.replace(/\*\*\*(.*?)\*\*\*/g, '<strong><em>$1</em></strong>');

  // 3. Bold: **text** -> <strong>text</strong>
  text = text.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');

  // 4. Standalone italic: *text* (when not preceded or followed by spaces/newlines)
  text = text.replace(/(^|[^*])\*(?!\s)([^*]+?)(?<!\s)\*(?=[^*]|$)/g, '$1<em>$2</em>');

  // 5. Clean up any remaining stray asterisks like *** or **
  text = text.replace(/\*{2,}/g, '');

  // 6. Bullet lists: lines starting with "* " or "- " -> "• "
  text = text.replace(/^[\s]*[*•\-]\s+(.*)$/gm, '<span class="bullet">•</span> $1');

  // 7. Section titles (e.g. "1. Sự kiện & Ngữ cảnh:", "2. Hành động chi tiết:")
  text = text.replace(/^([0-9]+\.\s+[^:\n]+:)/gm, '<span class="section-title">$1</span>');

  // 8. Convert newlines to line breaks
  text = text.replace(/\n/g, '<br/>');

  return text;
}

export function behaviorLabel(name: string): string {
  const raw = (name || '').trim();
  if (!raw) return 'Không xác định';
  const key = raw.toLowerCase().replace(/[\s-]+/g, '_');
  const map: Record<string, string> = {
    // Dangerous & Aggressive behaviors / expressions / poses
    fighting: 'Đánh nhau',
    aggression: 'Gây hấn',
    aggressive: 'Hung hăng',
    aggressive_behavior: 'Hành vi hung hăng',
    aggressive_expression: 'Biểu cảm hung hăng',
    angry_expression: 'Biểu cảm giận dữ',
    aggressive_posing: 'Tư thế hung hăng',
    angry: 'Giận dữ',
    violence: 'Bạo lực',
    school_violence: 'Bạo lực học đường',
    assault: 'Hành hung / Tấn công',
    threat: 'Đe dọa',
    threatening: 'Đe dọa',
    weapon: 'Vũ khí',
    weapon_use: 'Sử dụng vũ khí',
    weapon_holding: 'Cầm vũ khí',
    knife: 'Dao',
    gun: 'Súng',
    stick: 'Gậy',
    punching: 'Đấm',
    kicking: 'Đá',
    slapping: 'Tát',
    choking: 'Bóp cổ',
    strangling: 'Bóp cổ',
    grappling: 'Vật lộn',
    wrestling: 'Vật lộn',
    taunting: 'Khiêu khích',
    provoking: 'Khiêu khích',
    provocation: 'Khiêu khích',
    argument: 'Cãi vã',
    quarreling: 'Tranh cãi',
    pushing: 'Xô đẩy',
    shoving: 'Xô đẩy',
    vandalism: 'Phá hoại tài sản',
    theft: 'Trộm cắp',
    stealing: 'Trộm cắp',
    harassment: 'Quấy rối',
    no_hands_driving: 'Lái xe bỏ 2 tay',

    // Normal actions & postures
    walking: 'Đi bộ',
    standing: 'Đứng',
    sitting: 'Ngồi',
    studying: 'Ngồi học',
    talking: 'Nói chuyện',
    running: 'Chạy',
    falling: 'Ngã',
    driving: 'Lái xe',
    exercising: 'Tập thể dục',
    working: 'Làm việc',
    pointing: 'Chỉ tay',
    carrying_object: 'Mang vác vật',
    gesturing: 'Ra hiệu',
    gesture: 'Cử chỉ',
    thumbs_up: 'Giơ ngón cái',
    phone_use: 'Dùng điện thoại',
    smoking: 'Hút thuốc',
    hugging: 'Ôm',
    dancing: 'Nhảy múa',
    physical_contact: 'Tiếp xúc va chạm',
    verbal: 'Giao tiếp bằng lời',
    proximity: 'Cự ly gần',

    // Roles & Severity
    aggressor: 'Kẻ gây hấn',
    victim: 'Nạn nhân',
    bystander: 'Người đứng xem',
    participant: 'Người tham gia',
    normal: 'Bình thường',
    dangerous: 'Nguy hiểm',
    unanalyzed: 'Chưa phân tích',
  };

  if (map[key]) return map[key];

  // Smart fallback for composite expressions (e.g. angry_face, aggressive_look, etc.)
  const parts = key.split('_');
  const wordMap: Record<string, string> = {
    aggressive: 'hung hăng',
    angry: 'giận dữ',
    expression: 'biểu cảm',
    behavior: 'hành vi',
    posing: 'tư thế',
    pose: 'tư thế',
    look: 'ánh nhìn',
    face: 'nét mặt',
    gesture: 'cử chỉ',
    fighting: 'đánh nhau',
    violent: 'bạo lực',
    threatening: 'đe dọa',
  };

  if (parts.length === 2 && wordMap[parts[0]] && wordMap[parts[1]]) {
    const first = wordMap[parts[1]];
    const second = wordMap[parts[0]];
    const res = `${first} ${second}`;
    return res.charAt(0).toUpperCase() + res.slice(1);
  }

  return raw;
}

export function genderLabel(g: string): string {
  const key = (g || '').trim().toLowerCase();
  const map: Record<string, string> = { male: 'Nam', female: 'Nữ', unknown: 'Không rõ' };
  return map[key] ?? g;
}

export function colorLabel(c: string): string {
  const key = (c || '').trim().toLowerCase();
  const map: Record<string, string> = {
    red: 'đỏ', white: 'trắng', black: 'đen', blue: 'xanh dương',
    green: 'xanh lá', yellow: 'vàng', orange: 'cam', pink: 'hồng',
    purple: 'tím', brown: 'nâu', gray: 'xám', grey: 'xám',
    blonde: 'vàng', brunette: 'nâu', golden: 'vàng', dark: 'tối màu', light: 'sáng màu',
    unknown: 'không rõ',
  };
  return map[key] ?? c;
}

export function clothingTypeLabel(t: string): string {
  const key = (t || '').trim().toLowerCase();
  const map: Record<string, string> = {
    shirt: 'áo sơ mi',
    't-shirt': 'áo thun',
    hoodie: 'áo hoodie',
    jacket: 'áo khoác',
    dress: 'váy',
    uniform: 'đồng phục',
    sweater: 'áo len',
    'tank-top': 'áo ba lỗ',
  };
  return map[key] ?? t;
}
