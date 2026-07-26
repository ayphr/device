import packageJson from '../../package.json';

declare const __APP_BUILD_NUMBER__: string;

export const APP_BUILD_NUMBER = __APP_BUILD_NUMBER__;
export const APP_VERSION = APP_BUILD_NUMBER === 'In Dev' ? 'Dev' : packageJson.version;
export const APP_REPOSITORY_URL = 'https://github.com/ayphr/device';
