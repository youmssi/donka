import openapi from '../../openapi.json';
import en from './en.json';
import fr from './fr.json';

it.each([
  { locale: 'en', messages: en },
  { locale: 'fr', messages: fr },
])('has a visible $locale label for every action the API can return', ({ messages }) => {
  for (const action of openapi.components.schemas.Action.enum) {
    let value: unknown = messages.auditActions;
    for (const part of action.split('.')) {
      value = value !== null && typeof value === 'object' ? Reflect.get(value, part) : undefined;
    }
    expect(value, action).toEqual(expect.any(String));
    expect(String(value).trim(), action).not.toBe('');
  }
});
