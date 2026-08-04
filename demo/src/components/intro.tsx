import React from 'react';
import {interpolate, useCurrentFrame} from 'remotion';
import {COLORS, SANS} from '../theme';

export const Intro: React.FC = () => {
	const frame = useCurrentFrame();
	const opacity = interpolate(frame, [0, 20], [0, 1], {extrapolateRight: 'clamp'});
	const y = interpolate(frame, [10, 30], [50, 0], {extrapolateRight: 'clamp'});
	return (
		<div
			style={{
				display: 'flex',
				flexDirection: 'column',
				alignItems: 'center',
				gap: 18,
				opacity,
				transform: `translateY(${y}px)`,
				fontFamily: SANS,
			}}
		>
			<div style={{fontSize: 88, fontWeight: 800, color: COLORS.text, letterSpacing: 2}}>
				horcrux
			</div>
			<div style={{fontSize: 30, color: COLORS.dim}}>Phase 1 — SSS + shard crypto</div>
			<div style={{fontSize: 22, color: COLORS.faint, letterSpacing: 4}}>
				split &nbsp;·&nbsp; encrypt &nbsp;·&nbsp; reconstruct
			</div>
		</div>
	);
};
