/**
 * A local, offline password-strength estimate.
 *
 * This is a coarse heuristic -- character-class entropy, a common-password
 * blocklist, and a couple of pattern penalties -- not a full grammar-based
 * estimator like zxcvbn. It exists to give real, honest feedback where none
 * existed before (the master-password, join, and backup-password screens
 * previously enforced only a length minimum), not to claim a precision this
 * approach cannot deliver. See `docs/THREAT_MODEL.md` V-01.
 */

export type PasswordStrengthScore = 0 | 1 | 2 | 3 | 4;

export interface PasswordStrengthResult {
  score: PasswordStrengthScore;
  label: string;
  suggestions: string[];
}

// A small sample of passwords that appear at the top of every public
// breach-corpus frequency list. Catching these matters far more than any
// entropy calculation: a "P@ssw0rd1" scores well on charset diversity alone
// despite being one of the first guesses in any real attack.
const COMMON_PASSWORDS = new Set([
  "password",
  "password1",
  "password123",
  "12345678",
  "123456789",
  "1234567890",
  "qwertyui",
  "qwerty123",
  "letmein1",
  "welcome1",
  "iloveyou",
  "monkey123",
  "dragon123",
  "master123",
  "sunshine1",
  "princess1",
  "football1",
  "baseball1",
  "trustno1",
  "changeme1",
  "hunter22",
  "abc12345",
  "passw0rd",
  "admin1234",
  "correcthorsebatterystaple",
]);

function charsetSize(password: string): number {
  let size = 0;
  if (/[a-z]/.test(password)) size += 26;
  if (/[A-Z]/.test(password)) size += 26;
  if (/\d/.test(password)) size += 10;
  if (/[^a-zA-Z0-9]/.test(password)) size += 33;
  return size || 1;
}

function hasLongRun(password: string): boolean {
  return /(.)\1{3,}/.test(password);
}

function hasSequentialRun(password: string): boolean {
  const lower = password.toLowerCase();
  const alphabets = ["abcdefghijklmnopqrstuvwxyz", "01234567890"];
  return alphabets.some((alphabet) => {
    for (let i = 0; i <= alphabet.length - 4; i += 1) {
      if (lower.includes(alphabet.slice(i, i + 4))) return true;
    }
    return false;
  });
}

export function estimatePasswordStrength(password: string): PasswordStrengthResult {
  if (password.length === 0) {
    return { score: 0, label: "", suggestions: [] };
  }

  const normalized = password.toLowerCase().replace(/[^a-z0-9]/g, "");
  if (COMMON_PASSWORDS.has(password.toLowerCase()) || COMMON_PASSWORDS.has(normalized)) {
    return {
      score: 0,
      label: "Very weak",
      suggestions: ["This is one of the most commonly used passwords. Choose something unique."],
    };
  }

  let bits = password.length * Math.log2(charsetSize(password));
  const suggestions: string[] = [];

  if (hasLongRun(password)) {
    bits -= 10;
    suggestions.push("Avoid repeating the same character.");
  }
  if (hasSequentialRun(password)) {
    bits -= 10;
    suggestions.push('Avoid sequences like "abcd" or "1234".');
  }
  bits = Math.max(0, bits);

  if (password.length < 12) {
    suggestions.push("Longer is stronger -- 12+ characters is a good target.");
  }
  if (charsetSize(password) <= 26) {
    suggestions.push("Mix in numbers, symbols, or uppercase letters.");
  }

  let score: PasswordStrengthScore;
  let label: string;
  if (bits < 28) {
    score = 0;
    label = "Very weak";
  } else if (bits < 36) {
    score = 1;
    label = "Weak";
  } else if (bits < 60) {
    score = 2;
    label = "Fair";
  } else if (bits < 80) {
    score = 3;
    label = "Good";
  } else {
    score = 4;
    label = "Strong";
  }

  return { score, label, suggestions };
}
