const PACIFIC_TZ = "America/Los_Angeles";

const pacificPartsFormatter = new Intl.DateTimeFormat("en-US", {
  timeZone: PACIFIC_TZ,
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

const pacificTimeFormatter = new Intl.DateTimeFormat("en-US", {
  timeZone: PACIFIC_TZ,
  hour: "numeric",
  minute: "2-digit",
});

const pacificDateFormatter = new Intl.DateTimeFormat("en-US", {
  timeZone: PACIFIC_TZ,
  weekday: "short",
  month: "short",
  day: "numeric",
});

function getPart(
  parts: Intl.DateTimeFormatPart[],
  type: Intl.DateTimeFormatPartTypes,
): string {
  return parts.find((part) => part.type === type)?.value ?? "";
}

export function getPacificDateParts(date: Date = new Date()) {
  const parts = pacificPartsFormatter.formatToParts(date);
  return {
    year: getPart(parts, "year"),
    month: getPart(parts, "month"),
    day: getPart(parts, "day"),
    hour: getPart(parts, "hour"),
    minute: getPart(parts, "minute"),
    second: getPart(parts, "second"),
  };
}

export function getPacificDateString(date: Date = new Date()): string {
  const { year, month, day } = getPacificDateParts(date);
  return `${year}-${month}-${day}`;
}

export function getPacificTimeString(date: Date = new Date()): string {
  const { hour, minute, second } = getPacificDateParts(date);
  return `${hour}:${minute}:${second}`;
}

export function getPacificTimestamp(date: Date = new Date()): string {
  return `${getPacificDateString(date)}T${getPacificTimeString(date)}`;
}

export function formatPacificDateLabel(dateIso: string): string {
  const [year, month, day] = dateIso.split("-");
  if (!year || !month || !day) {
    return dateIso;
  }
  const safeDate = new Date(Date.UTC(Number(year), Number(month) - 1, Number(day), 12, 0, 0));
  if (Number.isNaN(safeDate.getTime())) {
    return dateIso;
  }
  return pacificDateFormatter.format(safeDate);
}

export function formatPacificTimeLabel(iso: string): string {
  if (!iso) return "";
  const hasTimezone = /Z$|[+-]\d{2}:\d{2}$/.test(iso);
  if (hasTimezone) {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) {
      return "";
    }
    return pacificTimeFormatter.format(date);
  }

  const timePart = iso.split("T")[1]?.slice(0, 5);
  if (!timePart) return "";
  const [hourStr, minute] = timePart.split(":");
  const hour = Number(hourStr);
  if (Number.isNaN(hour)) {
    return timePart;
  }
  const period = hour >= 12 ? "PM" : "AM";
  const hour12 = hour % 12 === 0 ? 12 : hour % 12;
  return `${hour12}:${minute} ${period}`;
}

export function isPacificTimezoneConfigured(): boolean {
  return pacificPartsFormatter.resolvedOptions().timeZone === PACIFIC_TZ;
}
