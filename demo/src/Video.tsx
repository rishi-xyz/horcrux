import React from 'react';
import {AbsoluteFill, Sequence} from 'remotion';
import {INTRO_FRAMES, schedule} from './steps';
import {Intro} from './components/intro';
import {StepView} from './StepView';

export const Video: React.FC = () => {
	const sched = schedule();
	return (
		<AbsoluteFill
			style={{
				background: 'radial-gradient(1400px 800px at 50% 16%, #1a2230, #0b0e14)',
				display: 'flex',
				alignItems: 'center',
				justifyContent: 'center',
			}}
		>
			<Sequence from={0} durationInFrames={INTRO_FRAMES}>
				<Intro />
			</Sequence>
			{sched.map((s, i) => (
				<Sequence key={i} from={s.start} durationInFrames={s.len}>
					<StepView index={i} step={s.step} />
				</Sequence>
			))}
		</AbsoluteFill>
	);
};
