import { CircleHelp, ShieldCheck } from "lucide-react";

const administratorHelp =
  "当录制或回放需要操作高权限的系统窗口时使用。重启后 Windows 会显示 UAC 确认；UAC 安全桌面仍需手动操作。";

interface AdministratorControlProps {
  isElevated: boolean;
  disabled: boolean;
  onRestart: () => void;
}

export function AdministratorControl({
  isElevated,
  disabled,
  onRestart
}: AdministratorControlProps) {
  return (
    <section className="administrator-control" aria-label="管理员模式">
      <button
        className="action-button administrator-restart-button"
        type="button"
        onClick={onRestart}
        disabled={disabled || isElevated}
      >
        <ShieldCheck size={16} aria-hidden="true" />
        <span className="button-label">
          {isElevated ? "已在管理员模式" : "以管理员身份重启"}
        </span>
      </button>
      <span
        className="administrator-help"
        tabIndex={0}
        aria-label="管理员模式说明"
        aria-describedby="administrator-help-text"
        data-tooltip={administratorHelp}
      >
        <CircleHelp size={16} aria-hidden="true" />
      </span>
      <span id="administrator-help-text" className="sr-only">
        {administratorHelp}
      </span>
    </section>
  );
}
