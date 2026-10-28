// The layout owns Send; the composer owns initialization and the send request.
export const COMPOSER_STATUS = Symbol("composer-status");
export interface ComposerStatus {
  ready: boolean;
  sending: boolean;
}
