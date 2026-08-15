import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Download,
  HardDrive,
  Loader2,
  Package,
  ShieldAlert,
  ShieldCheck,
  X,
} from "lucide-react";
import { t, tPlural } from "../i18n";
import { CommunicationAdapterPlanItem, WorkspaceInstallPlan } from "../types";

export interface AdapterConsentModalProps {
  show: boolean;
  plan: WorkspaceInstallPlan | null;
  loading?: boolean;
  onClose: () => void;
  onInstallAndCreate: () => void;
  onCreateWithoutInstalling: () => void;
}

export function AdapterConsentModal({
  show,
  plan,
  loading = false,
  onClose,
  onInstallAndCreate,
  onCreateWithoutInstalling,
}: AdapterConsentModalProps) {
  if (!show || !plan) return null;

  const hasManualMigration = plan.items.some(
    (item) => item.actionReason === "manual-migration-required"
  );
  const canApplyInstall = Boolean(plan.canApply && !hasManualMigration);
  const canCreateWithout = Boolean(plan.canCreateWithoutInstalling);

  const renderActionReason = (item: CommunicationAdapterPlanItem) => {
    switch (item.actionReason) {
      case "upgrade":
        return t("consent.actionReason.upgrade", { version: item.targetVersion });
      case "repair":
        return t("consent.actionReason.repair", { version: item.targetVersion });
      case "manual-migration-required":
        return t("consent.actionReason.manualMigration");
      case "install":
      default:
        return t("consent.actionReason.install", { version: item.targetVersion });
    }
  };

  const renderSourceLabel = (item: CommunicationAdapterPlanItem) => {
    switch (item.sourceKind) {
      case "bundled":
        return t("consent.source.bundled");
      case "npm-registry":
        return item.packageName
          ? t("consent.source.npmRegistry", { pkg: item.packageName })
          : "npm";
      case "pi-git":
        return t("consent.source.piGit");
      case "existing-global":
        return t("consent.source.existingGlobal");
      default:
        return t("consent.source.unknown");
    }
  };

  const renderConfigChangeInfo = (item: CommunicationAdapterPlanItem) => {
    switch (item.configChangeKind) {
      case "host-config-registered":
        return {
          icon: <AlertTriangle className="w-3 h-3 text-amber-400 shrink-0" />,
          text: t("consent.config.hostRegistered"),
          textColor: "text-amber-300",
        };
      case "app-private-managed":
        return {
          icon: <CheckCircle2 className="w-3 h-3 text-emerald-400 shrink-0" />,
          text: t("consent.config.appPrivate"),
          textColor: "text-slate-300",
        };
      case "none":
      default:
        return {
          icon: <CheckCircle2 className="w-3 h-3 text-emerald-400 shrink-0" />,
          text: t("consent.config.none"),
          textColor: "text-slate-300",
        };
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="consent-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 transition-opacity duration-150 motion-reduce:transition-none"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !loading) {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="w-full max-w-xl rounded-2xl bg-slate-900 border border-slate-700 shadow-2xl p-6 text-slate-100 flex flex-col max-h-[90vh] motion-reduce:transform-none">
        {/* Header */}
        <div className="flex items-start justify-between mb-3">
          <div className="flex items-center space-x-3">
            <div className="p-2 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-400 shrink-0">
              <ShieldAlert className="w-5 h-5" />
            </div>
            <div>
              <h3 id="consent-modal-title" className="text-base sm:text-lg font-bold text-slate-100">
                {t("consent.title")}
              </h3>
              <p className="text-xs text-slate-400 mt-0.5">
                {t("consent.description")}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={loading}
            aria-label={t("consent.actionCancel")}
            className="text-slate-400 hover:text-white transition cursor-pointer p-1 rounded-lg hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Status Layers Overview */}
        <div className="grid grid-cols-3 gap-2 py-2 px-3 mb-3.5 rounded-xl bg-slate-950/80 border border-slate-800 text-[11px]">
          <div className="flex flex-col">
            <span className="text-slate-500 text-[10px] uppercase font-mono tracking-wider">
              {t("status.layer.adapter")}
            </span>
            <span className="text-slate-200 font-medium truncate">
              {tPlural("consent.adapterChanges", plan.items.length)}
            </span>
          </div>
          <div className="flex flex-col">
            <span className="text-slate-500 text-[10px] uppercase font-mono tracking-wider">
              {t("status.layer.intercom")}
            </span>
            <span className="text-cyan-400 font-medium truncate">
              Protocol v4
            </span>
          </div>
          <div className="flex flex-col">
            <span className="text-slate-500 text-[10px] uppercase font-mono tracking-wider">
              {t("status.layer.team")}
            </span>
            <span className="text-amber-400 font-medium truncate">
              Auto-Team
            </span>
          </div>
        </div>

        {/* Manual Migration Callout if required */}
        {hasManualMigration && (
          <div className="flex items-start space-x-2 p-3 mb-3 rounded-xl bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300">
            <AlertCircle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
            <p className="leading-relaxed">
              {t("consent.manualMigrationHint")}
            </p>
          </div>
        )}

        {/* Aggregated Plan Item List */}
        <div className="flex-1 overflow-y-auto space-y-2.5 pr-1 max-h-[46vh]">
          {plan.items.map((item, idx) => {
            const configInfo = renderConfigChangeInfo(item);
            const isManualMigration = item.actionReason === "manual-migration-required";
            return (
              <div
                key={`${item.agentId}-${idx}`}
                className={`p-3.5 rounded-xl bg-slate-950 border transition ${
                  isManualMigration
                    ? "border-rose-800/80 bg-rose-950/20"
                    : "border-slate-800/90 hover:border-slate-700"
                }`}
              >
                <div className="flex items-center justify-between gap-2 mb-2">
                  <div className="flex items-center space-x-2 min-w-0">
                    <Package className="w-4 h-4 text-cyan-400 shrink-0" />
                    <span className="text-xs font-semibold text-slate-100 truncate">
                      {item.hostDisplayName || item.agentId}
                    </span>
                    <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-slate-800 text-slate-300 border border-slate-700">
                      {item.adapterKind}
                    </span>
                  </div>
                  <span
                    className={`px-2 py-0.5 rounded-md text-[10px] font-medium shrink-0 ${
                      isManualMigration
                        ? "bg-rose-500/20 border border-rose-500/40 text-rose-300"
                        : "bg-amber-500/15 border border-amber-500/30 text-amber-300"
                    }`}
                  >
                    {renderActionReason(item)}
                  </span>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-x-3 gap-y-1 text-[11px] text-slate-400 font-mono">
                  <div className="truncate">
                    <span className="text-slate-500">source: </span>
                    <span className="text-slate-300">{renderSourceLabel(item)}</span>
                  </div>
                  <div>
                    <span className="text-slate-500">ver: </span>
                    <span className="text-slate-300">
                      {item.installedVersion && item.installedVersion !== item.targetVersion
                        ? `v${item.installedVersion} → v${item.targetVersion}`
                        : `v${item.targetVersion}`}
                    </span>
                  </div>
                  <div className="flex items-center space-x-1">
                    {configInfo.icon}
                    <span className={configInfo.textColor}>
                      {configInfo.text}
                    </span>
                  </div>
                  <div className="flex items-center space-x-1">
                    {item.networkRequired ? (
                      <Download className="w-3 h-3 text-cyan-400 shrink-0" />
                    ) : (
                      <HardDrive className="w-3 h-3 text-emerald-400 shrink-0" />
                    )}
                    <span className="text-slate-300">
                      {item.networkRequired
                        ? t("consent.networkRequired")
                        : t("consent.offlineBundle")}
                    </span>
                  </div>
                </div>

                {item.license && (
                  <div className="mt-1.5 pt-1.5 border-t border-slate-900 text-[10px] text-slate-500 flex justify-between">
                    <span>{t("consent.license", { license: item.license })}</span>
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Security & Staging Rollback Notice */}
        <div className="flex items-start sm:items-center space-x-1.5 mt-3 pt-2 text-[11px] text-slate-400 border-t border-slate-800/80">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-400 shrink-0 mt-0.5 sm:mt-0" />
          <span className="leading-snug">{t("consent.securityGuarantee")}</span>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center justify-end space-x-2 mt-4 pt-2">
          <button
            type="button"
            onClick={onClose}
            disabled={loading}
            className="px-3 py-1.5 rounded-xl text-xs font-medium text-slate-400 hover:text-slate-200 transition cursor-pointer hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t("consent.actionCancel")}
          </button>
          <button
            type="button"
            onClick={onCreateWithoutInstalling}
            disabled={loading || !canCreateWithout}
            title={!canCreateWithout ? t("consent.disabledCreateWithout") : undefined}
            className="px-3.5 py-1.5 rounded-xl text-xs font-medium bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t("consent.actionWithout")}
          </button>
          <button
            type="button"
            onClick={onInstallAndCreate}
            disabled={loading || !canApplyInstall}
            title={!canApplyInstall ? t("consent.disabledInstall") : undefined}
            className="flex items-center space-x-1.5 px-4 py-1.5 rounded-xl text-xs font-semibold bg-cyan-600 hover:bg-cyan-500 text-white transition shadow-md shadow-cyan-900/30 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {loading ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>{t("consent.installing")}</span>
              </>
            ) : (
              <span>{t("consent.actionInstall")}</span>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
