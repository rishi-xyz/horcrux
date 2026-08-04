import React from 'react';
import {interpolate, useCurrentFrame} from 'remotion';
import {COLORS, MONO, SANS} from '../theme';

export const StepHeader: React.FC<{
	n: number;
	title: string;
	module: string;
	note: string;
}> = ({n, title, module, note}) => {
	const frame = useCurrentFrame();
	const opacity = interpolate(frame, [0, 12], [0, 1], {extrapolateRight: 'clamp'});
	const y = interpolate(frame, [0, 12], [-24, 0], {extrapolateRight: 'clamp'});
	return (
		<div
			style={{
				opacity,
				transform: `translateY(${y}px)`,
				width: 1320,
				display: 'flex',
				flexDirection: 'column',
				gap: 6,
				fontFamily: SANS,
			}}
		>
			<div style={{display: 'flex', alignItems: 'baseline', gap: 18}}>
				<span style={{color: COLORS.magenta, fontWeight: 800, fontSize: 28}}>STEP {n}</span>
				<span style={{color: COLORS.text, fontWeight: 700, fontSize: 36}}>{title}</span>
				<span style={{color: COLORS.faint, fontSize: 20, marginLeft: 'auto', fontFamily: MONO}}>
					{module}
				</span>
			</div>
			<div style={{color: COLORS.dim, fontSize: 22}}>{note}</div>
		</div>
	);
};
