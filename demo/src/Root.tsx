import React from 'react';
import {Composition} from 'remotion';
import {FPS, computeTotalFrames} from './steps';
import {Video} from './Video';

export const RemotionRoot: React.FC = () => (
	<Composition
		id="horcrux-demo"
		component={Video}
		durationInFrames={computeTotalFrames()}
		fps={FPS}
		width={1920}
		height={1080}
	/>
);
