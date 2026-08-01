// Rio 设备 SVG 矢量图
// 每个组件渲染真实设备的简化矢量外观
// 参考：Diamond Rio 500/600/800/S-Series/Karma 实物图

interface SvgProps {
  size?: number;
}

// Rio S50 - 蓝灰色运动型圆角矩形，LCD + 方向键
export function RioSSeries({ size = 80 }: SvgProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 机身 */}
      <rect
        x="20"
        y="10"
        width="60"
        height="80"
        rx="10"
        fill="url(#s-series-body)"
        stroke="#1a3a5c"
        strokeWidth="1.5"
      />
      {/* LCD 屏 */}
      <rect x="27" y="17" width="46" height="22" rx="2" fill="#3a5a7a" stroke="#0a1a2a" strokeWidth="0.8" />
      <rect x="29" y="19" width="42" height="18" rx="1" fill="#5a7a9a" opacity="0.6" />
      {/* LCD 内文字模拟 */}
      <rect x="31" y="22" width="20" height="2" fill="#0a1a2a" opacity="0.7" />
      <rect x="31" y="26" width="14" height="1.5" fill="#0a1a2a" opacity="0.5" />
      {/* 中央方向键 */}
      <circle cx="50" cy="60" r="11" fill="#2a4a6a" stroke="#0a1a2a" strokeWidth="0.8" />
      <circle cx="50" cy="60" r="4" fill="#5a7a9a" />
      {/* 四向小箭头 */}
      <path d="M 50 50 L 48 53 L 52 53 Z" fill="#0a1a2a" />
      <path d="M 50 70 L 48 67 L 52 67 Z" fill="#0a1a2a" />
      <path d="M 40 60 L 43 58 L 43 62 Z" fill="#0a1a2a" />
      <path d="M 60 60 L 57 58 L 57 62 Z" fill="#0a1a2a" />
      {/* Rio logo 区 */}
      <text x="50" y="86" textAnchor="middle" fontSize="6" fill="#ffffff" fontFamily="sans-serif" fontWeight="bold">Rio</text>
      {/* 渐变 */}
      <defs>
        <linearGradient id="s-series-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#3a6a9a" />
          <stop offset="100%" stopColor="#1a3a5c" />
        </linearGradient>
      </defs>
    </svg>
  );
}

// Rio S30S - 橙色卵圆形运动型（S30 Sport）
export function RioS30S({ size = 80 }: SvgProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 机身 - 卵圆形（横向椭圆，rx 较大） */}
      <rect
        x="14"
        y="18"
        width="72"
        height="64"
        rx="32"
        ry="32"
        fill="url(#s30s-body)"
        stroke="#8a3a00"
        strokeWidth="1.5"
      />
      {/* 顶部高光（运动型质感） */}
      <ellipse cx="50" cy="24" rx="28" ry="4" fill="#ffb87a" opacity="0.5" />
      {/* LCD 屏 - 中央偏上 */}
      <rect x="30" y="28" width="40" height="18" rx="3" fill="#1a2a3a" stroke="#000" strokeWidth="0.8" />
      <rect x="32" y="30" width="36" height="14" rx="1" fill="#5a8aaa" opacity="0.7" />
      <rect x="34" y="33" width="18" height="2" fill="#0a1a2a" opacity="0.7" />
      <rect x="34" y="37" width="12" height="1.5" fill="#0a1a2a" opacity="0.5" />
      {/* 中央方向键 - 圆形 */}
      <circle cx="50" cy="60" r="11" fill="#7a2a00" stroke="#3a1a00" strokeWidth="0.8" />
      <circle cx="50" cy="60" r="4" fill="#ffb87a" />
      {/* 四向小箭头 */}
      <path d="M 50 51 L 48 54 L 52 54 Z" fill="#3a1a00" />
      <path d="M 50 69 L 48 66 L 52 66 Z" fill="#3a1a00" />
      <path d="M 41 60 L 44 58 L 44 62 Z" fill="#3a1a00" />
      <path d="M 59 60 L 56 58 L 56 62 Z" fill="#3a1a00" />
      {/* 左右小按钮（运动型装饰） */}
      <circle cx="22" cy="50" r="2.5" fill="#5a1f00" />
      <circle cx="78" cy="50" r="2.5" fill="#5a1f00" />
      {/* Rio logo - 底部 */}
      <text x="50" y="78" textAnchor="middle" fontSize="5.5" fill="#ffffff" fontFamily="sans-serif" fontWeight="bold">Rio</text>
      {/* 渐变 - 橙色 */}
      <defs>
        <linearGradient id="s30s-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#ff8a3c" />
          <stop offset="50%" stopColor="#ff6a00" />
          <stop offset="100%" stopColor="#c44000" />
        </linearGradient>
      </defs>
    </svg>
  );
}

// Rio 500 - 圆角矩形，深蓝灰色，1999 年款
export function Rio500({ size = 80 }: SvgProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 机身 */}
      <rect
        x="18"
        y="12"
        width="64"
        height="76"
        rx="8"
        fill="url(#rio-500-body)"
        stroke="#1a1a2a"
        strokeWidth="1.5"
      />
      {/* 顶部 LCD */}
      <rect x="24" y="18" width="52" height="20" rx="2" fill="#2a3a4a" stroke="#0a0a1a" strokeWidth="0.8" />
      <rect x="26" y="20" width="48" height="16" rx="1" fill="#4a5a6a" opacity="0.7" />
      <rect x="28" y="23" width="22" height="2" fill="#0a0a1a" opacity="0.7" />
      <rect x="28" y="27" width="16" height="1.5" fill="#0a0a1a" opacity="0.5" />
      {/* 中央圆形方向键 */}
      <circle cx="50" cy="55" r="13" fill="#1a2a3a" stroke="#0a0a1a" strokeWidth="0.8" />
      <circle cx="50" cy="55" r="5" fill="#3a4a5a" />
      {/* 两侧小圆按钮 */}
      <circle cx="32" cy="55" r="3" fill="#1a2a3a" />
      <circle cx="68" cy="55" r="3" fill="#1a2a3a" />
      {/* 底部 Rio logo */}
      <text x="50" y="80" textAnchor="middle" fontSize="7" fill="#cccccc" fontFamily="sans-serif" fontWeight="bold">Rio</text>
      <defs>
        <linearGradient id="rio-500-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#3a4a5a" />
          <stop offset="100%" stopColor="#1a2a3a" />
        </linearGradient>
      </defs>
    </svg>
  );
}

// Rio 600 - 类方形，红色摇杆
export function Rio600({ size = 80 }: SvgProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 机身 - 椭圆形 */}
      <rect
        x="20"
        y="14"
        width="60"
        height="72"
        rx="14"
        fill="url(#rio-600-body)"
        stroke="#2a2a2a"
        strokeWidth="1.5"
      />
      {/* LCD */}
      <rect x="28" y="20" width="44" height="18" rx="2" fill="#1a2a3a" stroke="#0a0a1a" strokeWidth="0.8" />
      <rect x="30" y="22" width="40" height="14" rx="1" fill="#3a5a7a" opacity="0.7" />
      <rect x="32" y="25" width="18" height="2" fill="#0a0a1a" opacity="0.7" />
      {/* 红色 Rio Stick 摇杆 */}
      <circle cx="50" cy="56" r="10" fill="#1a1a1a" stroke="#000" strokeWidth="0.8" />
      <circle cx="50" cy="56" r="6" fill="#c92020" />
      <circle cx="50" cy="56" r="2" fill="#ff4040" />
      {/* 左右按钮 */}
      <rect x="28" y="52" width="8" height="8" rx="1.5" fill="#2a2a2a" stroke="#000" strokeWidth="0.5" />
      <rect x="64" y="52" width="8" height="8" rx="1.5" fill="#2a2a2a" stroke="#000" strokeWidth="0.5" />
      {/* 底部 nav 按钮 */}
      <circle cx="40" cy="74" r="3" fill="#2a2a2a" />
      <circle cx="60" cy="74" r="3" fill="#2a2a2a" />
      {/* Rio logo */}
      <text x="50" y="84" textAnchor="middle" fontSize="6" fill="#ffffff" fontFamily="sans-serif" fontWeight="bold">Rio</text>
      <defs>
        <linearGradient id="rio-600-body" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#3a3a3a" />
          <stop offset="100%" stopColor="#1a1a1a" />
        </linearGradient>
      </defs>
    </svg>
  );
}

// Rio 800 - 与 600 类似但更方正
export function Rio800({ size = 80 }: SvgProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 机身 - 方正 */}
      <rect
        x="20"
        y="12"
        width="60"
        height="76"
        rx="6"
        fill="url(#rio-800-body)"
        stroke="#2a2a2a"
        strokeWidth="1.5"
      />
      {/* 顶部 LCD - 较大 */}
      <rect x="26" y="18" width="48" height="24" rx="2" fill="#1a3a5a" stroke="#0a1a2a" strokeWidth="0.8" />
      <rect x="28" y="20" width="44" height="20" rx="1" fill="#3a6a9a" opacity="0.6" />
      <rect x="30" y="24" width="24" height="2" fill="#0a1a2a" opacity="0.7" />
      <rect x="30" y="28" width="18" height="1.5" fill="#0a1a2a" opacity="0.5" />
      <rect x="30" y="32" width="20" height="1.5" fill="#0a1a2a" opacity="0.4" />
      {/* 红色摇杆 */}
      <circle cx="50" cy="60" r="10" fill="#1a1a1a" stroke="#000" strokeWidth="0.8" />
      <circle cx="50" cy="60" r="6" fill="#a01818" />
      <circle cx="50" cy="60" r="2" fill="#ff3030" />
      {/* 左右银色按钮 */}
      <rect x="26" y="56" width="9" height="8" rx="1.5" fill="#5a5a5a" stroke="#000" strokeWidth="0.5" />
      <rect x="65" y="56" width="9" height="8" rx="1.5" fill="#5a5a5a" stroke="#000" strokeWidth="0.5" />
      {/* 底部小按钮 */}
      <circle cx="38" cy="78" r="2.5" fill="#3a3a3a" />
      <circle cx="50" cy="80" r="2.5" fill="#3a3a3a" />
      <circle cx="62" cy="78" r="2.5" fill="#3a3a3a" />
      {/* Rio logo */}
      <text x="50" y="84" textAnchor="middle" fontSize="0" fill="#fff"> </text>
      <defs>
        <linearGradient id="rio-800-body" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#4a4a4a" />
          <stop offset="100%" stopColor="#1a1a1a" />
        </linearGradient>
      </defs>
    </svg>
  );
}

// Rio Karma - 方正（77.5x79.5），黑色，红色 Rio Stick，右上角滚轮
export function RioKarma({ size = 80 }: SvgProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      {/* 机身 - 接近正方形，圆角 */}
      <rect
        x="14"
        y="14"
        width="72"
        height="72"
        rx="10"
        fill="url(#karma-body)"
        stroke="#000"
        strokeWidth="1.5"
      />
      {/* 顶部 Rio 字样 */}
      <text x="50" y="26" textAnchor="middle" fontSize="7" fill="#888" fontFamily="sans-serif" fontWeight="bold">Rio</text>
      {/* LCD - 中央偏下 */}
      <rect x="22" y="32" width="56" height="22" rx="2" fill="#0a1a2a" stroke="#000" strokeWidth="0.8" />
      <rect x="24" y="34" width="52" height="18" rx="1" fill="#3a6a8a" opacity="0.7" />
      <rect x="26" y="38" width="28" height="2" fill="#0a1a2a" opacity="0.8" />
      <rect x="26" y="42" width="20" height="1.5" fill="#0a1a2a" opacity="0.6" />
      {/* 红色 Rio Stick - 左下 */}
      <circle cx="35" cy="68" r="9" fill="#1a1a1a" stroke="#000" strokeWidth="0.8" />
      <circle cx="35" cy="68" r="5.5" fill="#c92020" />
      <circle cx="35" cy="68" r="2" fill="#ff4040" />
      {/* 右下 menu 按钮 */}
      <rect x="58" y="62" width="14" height="10" rx="2" fill="#2a2a2a" stroke="#000" strokeWidth="0.5" />
      <text x="65" y="69" textAnchor="middle" fontSize="5" fill="#888" fontFamily="sans-serif">menu</text>
      {/* 右上角滚轮 */}
      <circle cx="74" cy="40" r="6" fill="#2a2a2a" stroke="#000" strokeWidth="0.5" />
      <circle cx="74" cy="40" r="3.5" fill="#3a3a3a" />
      {/* 滚轮纹路 */}
      <line x1="74" y1="36" x2="74" y2="38" stroke="#000" strokeWidth="0.5" />
      <line x1="74" y1="42" x2="74" y2="44" stroke="#000" strokeWidth="0.5" />
      <defs>
        <linearGradient id="karma-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#2a2a2a" />
          <stop offset="100%" stopColor="#0a0a0a" />
        </linearGradient>
      </defs>
    </svg>
  );
}

export type DeviceModel = "s-series" | "s30s" | "rio-500" | "rio-600" | "rio-800" | "karma";

export function DeviceSvg({ model, size }: { model: DeviceModel; size?: number }) {
  switch (model) {
    case "s-series":
      return <RioSSeries size={size} />;
    case "s30s":
      return <RioS30S size={size} />;
    case "rio-500":
      return <Rio500 size={size} />;
    case "rio-600":
      return <Rio600 size={size} />;
    case "rio-800":
      return <Rio800 size={size} />;
    case "karma":
      return <RioKarma size={size} />;
  }
}
