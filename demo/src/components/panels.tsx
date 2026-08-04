import React from 'react';
import {useCurrentFrame} from 'remotion';
import type {VerifyRow} from '../steps';
import {COLORS, MONO} from '../theme';

export const Verify: React.FC<{rows: VerifyRow[]}> = ({rows}) => {
	const frame = useCurrentFrame();
	return (
		<div style={{display: 'flex', flexDirection: 'column', gap: 14, fontFamily: MONO}}>
			<div style={{color: COLORS.dim}}>comparing each reconstruction to the original key…</div>
			{rows.map((r, i) => {
				const at = 20 + i * 20;
				if (frame < at) return null;
				const opacity = Math.min(1, (frame - at) / 6);
				return (
					<div key={i} style={{display: 'flex', gap: 16, alignItems: 'baseline', opacity}}>
						<span style={{color: COLORS.green, fontWeight: 800}}>[ OK ]</span>
						<span style={{color: COLORS.text, width: 170}}>{r.label}</span>
						<span style={{color: COLORS.dim, fontSize: 19}}>
							{r.key.slice(0, 18)}…{r.key.slice(-6)}
						</span>
					</div>
				);
			})}
		</div>
	);
};

export const Summary: React.FC<{rows: string[]}> = ({rows}) => {
	const frame = useCurrentFrame();
	return (
		<div style={{display: 'flex', flexDirection: 'column', gap: 11, fontFamily: MONO, fontSize: 19}}>
			{rows.map((r, i) => {
				const at = 10 + i * 18;
				if (frame < at) return null;
				const opacity = Math.min(1, (frame - at) / 6);
				return (
					<div key={i} style={{color: COLORS.green, opacity}}>
						{r}
					</div>
				);
			})}
			<div style={{marginTop: 10, color: COLORS.dim}}>
				next up: Phase 2 — sign (reconstruct in RAM, broadcast via alloy)
			</div>
		</div>
	);
};
