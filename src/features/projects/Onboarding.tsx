import { motion } from "motion/react";
import { useState } from "react";
import { Button } from "../../components/Button";
import { Icons } from "../../components/icons";
import { LogoMark } from "../../components/Logo";
import { spring } from "../../design/motion";
import { useT } from "../../i18n";
import { notifyError } from "../../lib/notify";
import { addFolder } from "../dialogs/AddDialog";
import styles from "./Onboarding.module.css";

const icons = [Icons.layers, Icons.refresh, Icons.shield];

export function Onboarding() {
  const m = useT();
  const t = m.onboarding;
  const [busy, setBusy] = useState(false);

  const choose = async () => {
    setBusy(true);
    try {
      await addFolder();
    } catch (error) {
      notifyError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.onboarding}>
      <motion.div className={styles.mark} initial={{ opacity: 0, scale: 0.86 }} animate={{ opacity: 1, scale: 1 }} transition={spring.gentle}>
        <LogoMark size={64} weight="fine" />
      </motion.div>
      <motion.div className={styles.text} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ ...spring.gentle, delay: 0.08 }}>
        <h2>{t.welcome}</h2>
        <p>{t.body}</p>
        <Button variant="primary" size="lg" icon={<Icons.folderPlus size={16} />} loading={busy} onClick={() => void choose()}>
          {t.choose}
        </Button>
      </motion.div>
      <ul className={styles.features}>
        {t.features.map((feature, index) => {
          const Icon = icons[index] ?? Icons.check;
          return (
            <motion.li key={feature.title} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ ...spring.gentle, delay: 0.18 + index * 0.06 }}>
              <span className={styles.featureIcon}>
                <Icon size={15} />
              </span>
              <span>
                <span className={styles.featureTitle}>{feature.title}</span>
                <span className={styles.featureBody}>{feature.body}</span>
              </span>
            </motion.li>
          );
        })}
      </ul>
    </div>
  );
}
