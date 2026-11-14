import { useT } from "../i18n";
import { useGithubStore } from "../stores/github";
import { Button, type ButtonProps } from "./Button";
import { Icons } from "./icons";

export function InstallCliButton({ variant = "primary", size = "md" }: Pick<ButtonProps, "variant" | "size">) {
  const t = useT().github.connect;
  const installing = useGithubStore((state) => state.installing);
  const installCli = useGithubStore((state) => state.installCli);
  return (
    <Button variant={variant} size={size} loading={installing} icon={<Icons.download size={size === "lg" ? 15 : 13} />} onClick={() => void installCli()}>
      {installing ? t.installing : t.installAction}
    </Button>
  );
}
