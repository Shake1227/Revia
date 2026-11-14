import clsx from "clsx";
import { Button } from "../../components/Button";
import { Icons } from "../../components/icons";
import { clockLabel } from "../../components/lake";
import { Segmented } from "../../components/Segmented";
import { Slider } from "../../components/Slider";
import { Switch } from "../../components/Switch";
import { useT } from "../../i18n";
import { useAppStore, type BackgroundSettings, type SceneSetting } from "../../stores/app";
import { useBackgroundStore } from "../../stores/background";
import { useUiStore } from "../../stores/ui";
import { useSkyHour } from "../../app/useSkyHour";
import { Row, Section } from "../detail/SettingsTab";
import styles from "./BackgroundSection.module.css";

const SCENES: SceneSetting[] = ["lake", "hoshi", "plain", "image"];
export const TIME_PRESETS = [
  ["dawn", 5.5],
  ["morning", 7],
  ["noon", 12],
  ["evening", 17.5],
  ["sunset", 18.5],
  ["dusk", 19.5],
  ["midnight", 0],
] as const;

export function BackgroundSection() {
  const m = useT();
  const t = m.settings.background;
  const background = useAppStore((state) => state.ui.background);
  const theme = useAppStore((state) => state.ui.theme);
  const clock = useSkyHour(true, 0);
  const setUi = useAppStore((state) => state.setUi);
  const imageUrl = useBackgroundStore((state) => state.url);
  const busy = useBackgroundStore((state) => state.busy);
  const choose = useBackgroundStore((state) => state.choose);
  const remove = useBackgroundStore((state) => state.remove);

  const update = (patch: Partial<BackgroundSettings>) => setUi({ background: { ...background, ...patch } });
  const usesImage = background.scene === "image" && background.image !== null;

  const select = (scene: SceneSetting) => {
    if (scene === "image" && !background.image) void choose();
    else update({ scene });
  };

  return (
    <Section title={t.title} hint={t.hint}>
      <div className={styles.scenes} role="radiogroup" aria-label={t.title}>
        {SCENES.map((scene) => {
          const selected = background.scene === scene;
          return (
            <button key={scene} type="button" role="radio" aria-checked={selected} className={styles.scene} data-selected={selected || undefined} onClick={() => select(scene)}>
              <span
                className={clsx(styles.preview, styles[scene])}
                style={scene === "image" && imageUrl ? { backgroundImage: `url("${imageUrl}")` } : undefined}
              >
                {scene === "image" && !imageUrl && <Icons.plus size={16} />}
              </span>
              <span className={styles.sceneLabel}>{t.scenes[scene]}</span>
            </button>
          );
        })}
      </div>

      {background.scene === "lake" && (
        <>
          <Row label={t.time} hint={background.timeAuto ? t.timeAutoHint(clockLabel(clock)) : t.timeFixedHint}>
            <Segmented<"auto" | "fixed">
              value={background.timeAuto ? "auto" : "fixed"}
              onChange={(mode) => update({ timeAuto: mode === "auto", hour: mode === "fixed" && background.timeAuto ? Math.round(clock * 60) / 60 : background.hour })}
              options={[
                { value: "auto", label: t.timeAuto },
                { value: "fixed", label: t.timeFixed },
              ]}
            />
          </Row>
          {!background.timeAuto && (
            <>
              <Row label={t.hour}>
                <Slider
                  value={Math.round(background.hour * 60)}
                  onChange={(value) => update({ hour: value / 60 })}
                  min={0}
                  max={1439}
                  step={1}
                  ariaLabel={t.hour}
                  display={clockLabel(background.hour)}
                />
              </Row>
              <div className={styles.presets}>
                {TIME_PRESETS.map(([name, hour]) => (
                  <Button key={name} size="sm" variant={Math.abs(background.hour - hour) < 0.009 ? "secondary" : "ghost"} onClick={() => update({ hour })}>
                    {t.presets[name]}
                  </Button>
                ))}
              </div>
            </>
          )}
        </>
      )}

      <Row label={t.image} hint={background.image ? background.image.name : t.imageHint}>
        <Button size="sm" icon={<Icons.folderOpen size={13} />} loading={busy} onClick={() => void choose()}>
          {background.image ? t.changeImage : t.chooseImage}
        </Button>
        {background.image && (
          <>
            <Button size="sm" onClick={() => useUiStore.getState().openDialog({ type: "backdropFrame" })}>
              {t.frame.adjust}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void remove()}>
              {t.removeImage}
            </Button>
          </>
        )}
      </Row>

      {usesImage && (
        <>
          <Row label={theme === "light" ? t.dimLight : t.dim}>
            <Slider value={background.dim} onChange={(dim) => update({ dim })} min={0} max={90} step={5} ariaLabel={t.dim} display={`${background.dim}%`} />
          </Row>
          <Row label={t.blur}>
            <Slider value={background.blur} onChange={(blur) => update({ blur })} min={0} max={40} step={2} ariaLabel={t.blur} display={`${background.blur}px`} />
          </Row>
        </>
      )}

      <Row label={t.panel} hint={t.panelHint}>
        <Slider value={background.panel} onChange={(panel) => update({ panel })} min={50} max={100} step={2} ariaLabel={t.panel} display={`${background.panel}%`} />
      </Row>
      <Row label={t.animate} hint={t.animateHint}>
        <Switch checked={background.animate} onChange={(animate) => update({ animate })} ariaLabel={t.animate} />
      </Row>
    </Section>
  );
}
