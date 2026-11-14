import { useEffect } from "react";
import { useOperationStore } from "../../stores/operations";
import { useUiStore } from "../../stores/ui";
import { AddDialog } from "./AddDialog";
import { BackdropFrameDialog } from "./BackdropFrameDialog";
import { CloneDialog } from "./CloneDialog";
import { LicenseDialog } from "./LicenseDialog";
import { LoginDialog } from "./LoginDialog";
import { MergeDialog } from "./MergeDialog";
import { BatchMergeDialog, DivergedDialog, LinkDialog, TargetDialog } from "./MoreDialogs";
import { PublishDialog } from "./PublishDialog";
import { DeleteDialog, UnlinkDialog } from "./RemovalDialogs";

export function GlobalDialogs() {
  const dialog = useUiStore((state) => state.dialog);
  const close = useUiStore((state) => state.closeDialog);

  useEffect(() => {
    if (dialog?.type !== "batchCommit") return;
    const ids = dialog.projectIds;
    close();
    void useOperationStore.getState().prepare({
      kind: "commit",
      items: ids.map((id) => ({ itemId: id, projectId: id, message: "-", paths: null })),
    });
  }, [dialog, close]);

  return (
    <>
      <AddDialog open={dialog?.type === "add"} onClose={close} />
      <LoginDialog open={dialog?.type === "login"} onClose={close} />
      <LicenseDialog open={dialog?.type === "license"} onClose={close} />
      <BackdropFrameDialog open={dialog?.type === "backdropFrame"} onClose={close} />
      <CloneDialog repo={dialog?.type === "clone" ? dialog.repo : null} onClose={close} />
      <PublishDialog projectIds={dialog?.type === "publish" ? dialog.projectIds : null} onClose={close} />
      <BatchMergeDialog projectIds={dialog?.type === "batchMerge" ? dialog.projectIds : null} onClose={close} />
      <DivergedDialog projectId={dialog?.type === "diverged" ? dialog.projectId : null} onClose={close} />
      <TargetDialog projectId={dialog?.type === "target" ? dialog.projectId : null} onClose={close} />
      <LinkDialog projectId={dialog?.type === "link" ? dialog.projectId : null} onClose={close} />
      <MergeDialog request={dialog?.type === "merge" ? dialog : null} onClose={close} />
      <UnlinkDialog projectId={dialog?.type === "unlink" ? dialog.projectId : null} onClose={close} />
      <DeleteDialog projectId={dialog?.type === "delete" ? dialog.projectId : null} onClose={close} />
    </>
  );
}
