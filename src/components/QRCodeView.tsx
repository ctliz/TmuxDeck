import { useState, useEffect } from "react";
import QRCode from "qrcode";

interface QRCodeViewProps {
  text: string;
  size?: number;
}

export function QRCodeView({ text, size = 190 }: QRCodeViewProps) {
  const [svgStr, setSvgStr] = useState<string>("");

  useEffect(() => {
    if (!text) return;
    QRCode.toString(text, { type: "svg", margin: 1, width: size })
      .then((res) => setSvgStr(res))
      .catch((err) => console.error("QR Code generation error:", err));
  }, [text, size]);

  if (!svgStr) return null;

  return (
    <div
      className="inline-block p-3 bg-white rounded-xl shadow-lg border border-slate-200/50"
      dangerouslySetInnerHTML={{ __html: svgStr }}
    />
  );
}
