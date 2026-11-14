import licenseText from "../../../LICENSE?raw";
import { LICENSE } from "../../app/meta";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { Icons } from "../../components/icons";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { notifyError } from "../../lib/notify";
import styles from "./LicenseDialog.module.css";

export function LicenseDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const m = useT();
  const t = m.settings.about;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      icon={<Icons.file size={16} />}
      title={`${t.license}${m.common.dot}${LICENSE.name}`}
      description={t.licenseBody}
      size="lg"
      flush
      footerStart={
        <Button
          size="sm"
          variant="ghost"
          icon={<Icons.external size={13} />}
          onClick={() => void api.openUrl({ url: LICENSE.url }).catch(notifyError)}
        >
          {t.licenseOfficial}
        </Button>
      }
      footer={
        <Button variant="primary" onClick={onClose}>
          {m.common.close}
        </Button>
      }
    >
      <p className={styles.note}>{t.licenseTextNote}</p>
      <pre className={`${styles.text} mono selectable`} tabIndex={0}>
        {licenseText}
      </pre>
    </Dialog>
  );
}
