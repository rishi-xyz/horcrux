import { NavLink, Route, Routes, useNavigate } from "react-router-dom";
import { hasToken } from "./api";
import { LogPage } from "./pages/LogPage";
import { VerifyPage } from "./pages/VerifyPage";
import { InitPage } from "./pages/InitPage";
import { SignPage } from "./pages/SignPage";
import { MpcPage } from "./pages/MpcPage";

const NAV = [
  { to: "/log", icon: "⏱", label: "Access log", tag: "" },
  { to: "/verify", icon: "✓", label: "Verify", tag: "" },
  { to: "/init", icon: "✂", label: "Init", tag: "Mode A split" },
  { to: "/sign", icon: "✎", label: "Sign", tag: "Mode A" },
  { to: "/mpc", icon: "◈", label: "MPC", tag: "Mode B" },
];

export default function App() {
  const navigate = useNavigate();
  const tokenOk = hasToken();

  return (
    <div className="shell">
      <aside className="rail">
        <div className="rail-brand">
          <svg viewBox="0 0 32 32" width="22" height="22">
            <path
              d="M16 4l10 5v7c0 7.4-4.4 11.9-10 12.6C10.4 27.9 6 23.4 6 16V9l10-5z"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            />
          </svg>
          <span>HORCRUX</span>
        </div>
        <p className="rail-tag">Local console · Solana/Bitcoin/Cosmos</p>
        <nav className="rail-nav">
          {NAV.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) => "rail-item" + (isActive ? " is-active" : "")}
            >
              <span className="rail-ic">{item.icon}</span> {item.label}
              {item.tag && <em>{item.tag}</em>}
            </NavLink>
          ))}
        </nav>
        <div className="rail-footer">
          <div className={"token-state" + (tokenOk ? "" : " is-bad")}>
            {tokenOk ? "token loaded" : "no token in URL — every action will 401"}
          </div>
          <p className="rail-hint">
            Bound to 127.0.0.1 only. Paths below are read from disk on this machine, never uploaded
            from the browser (except QR imports, which stay on this machine too).
          </p>
        </div>
      </aside>

      <main className="content">
        <div className="topbar">
          <button className="btn btn-outline" type="button" onClick={() => navigate(-1)} title="Go back">
            ‹ Back
          </button>
        </div>
        <Routes>
          <Route path="/" element={<LogPage />} />
          <Route path="/log" element={<LogPage />} />
          <Route path="/verify" element={<VerifyPage />} />
          <Route path="/init" element={<InitPage />} />
          <Route path="/sign" element={<SignPage />} />
          <Route path="/mpc" element={<MpcPage />} />
          <Route path="/mpc/:tab" element={<MpcPage />} />
        </Routes>
      </main>
    </div>
  );
}
