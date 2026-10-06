import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";

export function CopyValue({ value }: { value: string }) {
  const [message, setMessage] = useState("");

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setMessage("已复制");
    } catch {
      setMessage("复制失败，请选中文本手动复制");
    }
  }

  return (
    <div className="group/value min-w-0">
      <div className="flex min-w-0 items-center gap-2 rounded-md bg-muted px-3 py-1.5">
        <code className="min-w-0 flex-1 select-text break-all font-mono text-xs leading-5">
          {value}
        </code>
        <Button
          size="icon-sm"
          variant="ghost"
          aria-label={`复制 ${value}`}
          title="复制完整值"
          onClick={() => void copy()}
          onBlur={() => setMessage("")}
        >
          {message === "已复制" ? (
            <Check aria-hidden="true" className="text-success" />
          ) : (
            <Copy aria-hidden="true" />
          )}
        </Button>
      </div>
      <span role="status" className="sr-only">
        {message}
      </span>
      {message.startsWith("复制失败") ? (
        <p className="mt-1 text-xs text-warning">{message}</p>
      ) : null}
    </div>
  );
}
