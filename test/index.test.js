import test from 'node:test'
import assert from 'node:assert/strict'

import { I2CBusMahalli } from '@johntalton/i2c-bus-mahalli'

test('exports I2CBusMahalli class', () => {
  assert.equal(typeof I2CBusMahalli, 'function')
});

test('throws expected error when opening bus device', () => {
  if (process.platform === 'linux') {
    assert.throws(
      () => new I2CBusMahalli(99), {
      message: '[Io(Os { code: 2, kind: NotFound, message: "No such file or directory" })] Failed to open /dev/i2c-99: No such file or directory (os error 2)',
      name: 'Error'
    });
  } else {
    assert.throws(
      () => new I2CBusMahalli(99),
      /i2cdev is only supported on Linux/,
    )
  }
})

