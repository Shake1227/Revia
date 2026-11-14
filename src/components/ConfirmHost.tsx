import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { useConfirmStore, type ConfirmRequest } from "../stores/confirm";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import styles from "./ConfirmHost.module.css";
import { Dialog } from "./Dialog";
import { TextField } from "./Field";
import { Icons } from "./icons";

export function ConfirmHost() {
  const m = useT();
  const current = useConfirmStore((state) => state.current);
  const settle = useConfirmStore((state) => state.settle);
  const [shown, setShown] = useState<ConfirmRequest | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const [typed, setTyped] = useState("");
  const [options, setOptions] = useState<Record<string, boolean>>({});

  useEffect(() => {
    if (!current) return;
    setShown(current);
    setAcknowledged(false);
    setTyped("");
    setOptions(Object.fromEntries((current.options ?? []).map((option) => [option.id, option.defaultChecked ?? false])));
  }, [current]);

  const request = current ?? shown;
  if (!request) return null;

  const blocked =
    (request.acknowledge && !acknowledged) || (request.typeToConfirm !== undefined && typed.trim() !== request.typeToConfirm);
  const danger = request.level === "danger";

  return (
    <Dialog
      open={current !== null}
      onOpenChange={(open) => !open && settle({ confirmed: false, choice: null, options })}
      title={request.title}
      description={request.body}
      size="sm"
      tone={danger ? "danger" : "default"}
      icon={
        request.level === "safe" ? undefined : (
          <span className={styles.badge} data-level={request.level}>
            <Icons.warning size={15} />
          </span>
        )
      }
      footerStart={
        request.alternative && (
          <Button variant="ghost" size="md" onClick={() => settle({ confirmed: false, choice: request.alternative!.value, options })}>
            {request.alternative.label}
          </Button>
        )
      }
      footer={
        <>
          <Button variant="ghost" onClick={() => settle({ confirmed: false, choice: null, options })}>
            {request.cancelLabel ?? m.common.cancel}
          </Button>
          <Button
            variant={danger ? "dangerSolid" : "primary"}
            disabled={blocked}
            autoFocus={!danger}
            onClick={() => settle({ confirmed: true, choice: null, options })}
          >
            {request.confirmLabel}
          </Button>
        </>
      }
    >
      <div className={styles.content}>
        {request.facts && request.facts.length > 0 && (
          <dl className={styles.facts}>
            {request.facts.map((fact) => (
              <div key={fact.label} className={styles.fact}>
                <dt>{fact.label}</dt>
                <dd>{fact.value}</dd>
              </div>
            ))}
          </dl>
        )}
        {request.content}
        {request.risk && (
          <div className={styles.risk} data-level={request.level}>
            <Icons.warning size={14} />
            <div>{request.risk}</div>
          </div>
        )}
        {request.options?.map((option) => (
          <Checkbox
            key={option.id}
            checked={options[option.id] ?? false}
            onChange={(checked) => setOptions((previous) => ({ ...previous, [option.id]: checked }))}
            label={option.label}
            hint={option.hint}
          />
        ))}
        {request.acknowledge && <Checkbox checked={acknowledged} onChange={setAcknowledged} label={m.confirm.acknowledge} />}
        {request.typeToConfirm !== undefined && (
          <label className={styles.type}>
            <span>{m.confirm.typeToConfirm(request.typeToConfirm)}</span>
            <TextField value={typed} onChange={(event) => setTyped(event.target.value)} autoFocus placeholder={request.typeToConfirm} />
          </label>
        )}
      </div>
    </Dialog>
  );
}
