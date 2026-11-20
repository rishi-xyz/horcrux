/// The audit Block/Warn modal a Sign/MPC-Sign attempt must clear before
/// signing proceeds — mirrors the TUI's `render_audit_modal` and the CLI's
/// pre-flight bail/warn, restating the consequence of each choice so it's
/// hard to get stuck on.
export function AuditModal({
  blocking,
  reasons,
  onForce,
  onContinue,
  onCancel,
}: {
  blocking: boolean;
  reasons: string[];
  onForce: () => void;
  onContinue: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="modal-backdrop" onClick={onCancel}>
      <div
        className={"modal-card " + (blocking ? "is-blocking" : "is-warning")}
        onClick={(e) => e.stopPropagation()}
      >
        <div className={"modal-banner " + (blocking ? "is-blocking" : "is-warning")}>
          {blocking ? "⚠ AUDIT BLOCKED — signing refused" : "⚠ AUDIT WARNING — review before continuing"}
        </div>
        <ul className="modal-reasons">
          {reasons.map((r, i) => (
            <li key={i}>{r}</li>
          ))}
        </ul>
        <div className="modal-consequence">
          {blocking
            ? "This attempt is already logged; forcing through logs it again as forced."
            : "Continuing proceeds immediately; nothing is blocked."}
        </div>
        <div className="modal-actions">
          <button className="btn btn-outline" type="button" onClick={onCancel}>
            Cancel
          </button>
          {blocking ? (
            <button className="btn btn-primary" type="button" onClick={onForce}>
              Force through
            </button>
          ) : (
            <button className="btn btn-primary" type="button" onClick={onContinue}>
              Continue
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
