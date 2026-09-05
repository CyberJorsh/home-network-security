// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import SafeResponse from './SafeResponse';
afterEach(cleanup);
it('formats explanations without activating model HTML, links or remote images', () => {
  const { container } = render(
    <SafeResponse
      text={
        '## Observations\n\n- **SSH** was observed\n- `22/tcp`\n\n<img src="https://example.com/tracker">\n\n[Run](javascript:alert(1))\n\n```sh\ncurl example.com\n```'
      }
    />,
  );
  expect(
    screen.queryByRole('heading', { name: 'Observations' }),
  ).not.toBeNull();
  expect(container.querySelectorAll('li')).toHaveLength(2);
  expect(container.querySelectorAll('strong')).toHaveLength(1);
  expect(container.querySelectorAll('img,a,script')).toHaveLength(0);
  expect(container.querySelector('pre')?.textContent).toContain(
    'curl example.com',
  );
});
