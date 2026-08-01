/**
 * Compatibility export for older feature code.
 * The application now uses StoryRepository directly; browser persistence lives
 * only inside BrowserDemoRepository and is never used by the desktop runtime.
 */
export { BrowserDemoRepository } from './storyRepository';
