import { formatFullDate } from "../../utils/dateFormatting";
import type { SwitchRecord, UiCopy } from "../accounts/types";

export function ClassicRecentSwitches({
  records,
  locale,
  text,
}: {
  records: SwitchRecord[];
  locale: string;
  text: UiCopy;
}) {
  return (
    <section className={`detailCard recentSwitchCard${records.length === 0 ? " isEmpty" : ""}`}>
      <div className="detailSectionTitle">
        <h3>{text.recentSwitches}</h3>
      </div>
      {records.length > 0 ? (
        <ul className="recentSwitchList">
          {records.map((record) => (
            <li key={record.id}>
              <span className="statusDot status-using" />
              <span>{text.switchRecordAction}</span>
              <strong>{formatFullDate(record.timestamp, locale, text.emptyValue)}</strong>
              <em>
                {record.source === text.emptyValue
                  ? record.target
                  : `${text.fromPrefix} ${record.source}`}
              </em>
            </li>
          ))}
        </ul>
      ) : (
        <p className="recentSwitchEmpty">{text.noSwitchRecords}</p>
      )}
    </section>
  );
}
