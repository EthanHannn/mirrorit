import { AlertDialog } from "radix-ui";
import { type ReactNode, useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { ConfirmContext, type ConfirmOptions } from "@/hooks/use-confirm";

type ConfirmRequest = ConfirmOptions & {
  resolve: (confirmed: boolean) => void;
};

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [request, setRequest] = useState<ConfirmRequest | null>(null);

  const confirm = useCallback(
    (options: ConfirmOptions) =>
      new Promise<boolean>((resolve) => {
        setRequest({ ...options, resolve });
      }),
    [],
  );

  const settle = (confirmed: boolean) => {
    request?.resolve(confirmed);
    setRequest(null);
  };

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      <AlertDialog.Root
        onOpenChange={(open) => {
          if (!open) {
            settle(false);
          }
        }}
        open={request !== null}
      >
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 z-40 bg-black/45" />
          <AlertDialog.Content className="fixed top-1/2 left-1/2 z-50 w-[min(24rem,calc(100vw-2rem))] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-popover p-5 text-popover-foreground shadow-xl outline-none">
            <AlertDialog.Title className="text-base font-semibold">
              {request?.title}
            </AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-sm leading-6 text-muted-foreground">
              {request?.description}
            </AlertDialog.Description>
            <div className="mt-5 flex justify-end gap-2">
              <AlertDialog.Cancel asChild>
                <Button onClick={() => settle(false)} variant="outline">
                  {request?.cancelLabel ?? "取消"}
                </Button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <Button onClick={() => settle(true)}>
                  {request?.confirmLabel ?? "确认"}
                </Button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </ConfirmContext.Provider>
  );
}
