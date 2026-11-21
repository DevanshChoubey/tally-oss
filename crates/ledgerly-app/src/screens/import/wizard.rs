//! The wizard's state machine (no I/O; see the module documentation).

use ledgerly_core::{CancelToken, Date, FinancialYear, Period, PeriodError};

use super::types::{
    DetectError, DetectState, ImportError, ImportRequest, ImportSource, MapReport, MapState,
    PullProgress, PullState, PullSummary, ReconReport, ReconState, Step, TALLY_PORT,
};

/// How many financial years the period pickers offer, ending with the current one.
const FY_CHOICES: i32 = 5;

// ---------------------------------------------------------------------------------------------
// The wizard
// ---------------------------------------------------------------------------------------------

/// The wizard's state machine. It holds no I/O; see the module documentation.
#[derive(Debug)]
pub struct ImportWizard {
    step: Step,
    fy_options: Vec<FinancialYear>,
    first_fy: FinancialYear,
    last_fy: FinancialYear,
    source: ImportSource,
    detect: DetectState,
    companies: Vec<String>,
    selected: Option<usize>,
    pull: PullState,
    cancel_token: CancelToken,
    map: MapState,
    recon: ReconState,
    requests: Vec<ImportRequest>,
}

impl ImportWizard {
    /// A wizard at the first step. `today` picks the financial years on offer: the current one
    /// and the four before it; the current one is preselected.
    ///
    /// # Errors
    /// [`PeriodError::OutOfRange`] when `today` is at the edge of the supported calendar.
    pub fn new(today: Date) -> Result<Self, PeriodError> {
        let current = FinancialYear::containing(today)?;
        let mut fy_options = Vec::new();
        for back in (0..FY_CHOICES).rev() {
            let year = current
                .start
                .year()
                .checked_sub(back)
                .ok_or(PeriodError::OutOfRange)?;
            fy_options.push(FinancialYear::april(year)?);
        }
        Ok(ImportWizard {
            step: Step::Detect,
            fy_options,
            first_fy: current,
            last_fy: current,
            source: ImportSource::Live { port: TALLY_PORT },
            detect: DetectState::NotChecked,
            companies: Vec::new(),
            selected: None,
            pull: PullState::Idle,
            cancel_token: CancelToken::new(),
            map: MapState::NotStarted,
            recon: ReconState::NotStarted,
            requests: Vec::new(),
        })
    }

    /// The current step.
    #[must_use]
    pub fn step(&self) -> Step {
        self.step
    }

    /// What detect knows.
    #[must_use]
    pub fn detect_state(&self) -> &DetectState {
        &self.detect
    }

    /// The source the wizard reads from.
    #[must_use]
    pub fn source(&self) -> &ImportSource {
        &self.source
    }

    /// The companies the source listed, in the order it gave them.
    #[must_use]
    pub fn companies(&self) -> &[String] {
        &self.companies
    }

    /// The chosen company number, if any.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The financial years on offer, oldest first.
    #[must_use]
    pub fn fy_options(&self) -> &[FinancialYear] {
        &self.fy_options
    }

    /// The first and last financial year chosen.
    #[must_use]
    pub fn years(&self) -> (FinancialYear, FinancialYear) {
        (self.first_fy, self.last_fy)
    }

    /// The pull step's state.
    #[must_use]
    pub fn pull_state(&self) -> &PullState {
        &self.pull
    }

    /// The map step's state.
    #[must_use]
    pub fn map_state(&self) -> &MapState {
        &self.map
    }

    /// The reconcile step's state.
    #[must_use]
    pub fn recon_state(&self) -> &ReconState {
        &self.recon
    }

    /// The period from the start of the first chosen year to the end of the last.
    ///
    /// # Errors
    /// [`ImportError::Period`] when the first year is after the last or a date is out of range.
    pub fn period(&self) -> Result<Period, ImportError> {
        let to = self.last_fy.end()?;
        Ok(Period::new(self.first_fy.start, to)?)
    }

    /// Takes the work the wizard wants done, oldest first.
    pub fn take_requests(&mut self) -> Vec<ImportRequest> {
        std::mem::take(&mut self.requests)
    }

    /// `true` when the user can leave the screen without interrupting work that cannot be
    /// stopped half way.
    #[must_use]
    pub fn can_leave(&self) -> bool {
        let pulling = matches!(self.pull, PullState::Running(_) | PullState::Cancelling(_));
        let mapping = self.step == Step::Map && self.map == MapState::Working;
        let committing = self.step == Step::Reconcile && self.recon == ReconState::Committing;
        !(pulling || mapping || committing)
    }

    /// Stops any request that is still running because the screen is closing (rule D2).
    pub fn close(&mut self) {
        self.cancel_token.cancel();
    }

    fn wrong_step(&self, action: &'static str) -> ImportError {
        ImportError::WrongStep {
            action,
            step: self.step.label(),
        }
    }

    fn selected_company(&self) -> Result<&str, ImportError> {
        self.selected
            .and_then(|i| self.companies.get(i))
            .map(String::as_str)
            .ok_or(ImportError::NoCompany)
    }

    // ---- detect ----

    /// Asks the host to list the companies of `source`.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when not at the detect step or a request is already out.
    pub fn start_detect(&mut self, source: ImportSource) -> Result<(), ImportError> {
        if self.step != Step::Detect || self.detect == DetectState::Checking {
            return Err(self.wrong_step("look for companies"));
        }
        self.source = source.clone();
        self.detect = DetectState::Checking;
        self.requests.push(ImportRequest::Detect { source });
        Ok(())
    }

    /// The host's answer to a detect request. Companies move the wizard to the next step; an
    /// empty list is a failure, not an empty choice.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no detect request is out.
    pub fn detect_result(
        &mut self,
        result: Result<Vec<String>, DetectError>,
    ) -> Result<(), ImportError> {
        if self.step != Step::Detect || self.detect != DetectState::Checking {
            return Err(self.wrong_step("accept the list of companies"));
        }
        match result {
            Ok(list) if list.is_empty() => {
                self.detect = DetectState::Failed(
                    "the source lists no company; open one in the other app, or choose another file"
                        .to_owned(),
                );
            }
            Ok(list) => {
                self.companies = list;
                self.selected = None;
                self.detect = DetectState::Found;
                self.step = Step::ChooseCompany;
            }
            Err(DetectError::NotRunning { port }) => {
                self.detect = DetectState::NotRunning { port };
            }
            Err(DetectError::Other(reason)) => self.detect = DetectState::Failed(reason),
        }
        Ok(())
    }

    // ---- choose company ----

    /// Chooses a company by its number in [`ImportWizard::companies`].
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] outside the choose step, [`ImportError::CompanyIndex`] for a
    /// number that is not in the list.
    pub fn select_company(&mut self, index: usize) -> Result<(), ImportError> {
        if self.step != Step::ChooseCompany {
            return Err(self.wrong_step("choose a company"));
        }
        if index >= self.companies.len() {
            return Err(ImportError::CompanyIndex {
                index,
                count: self.companies.len(),
            });
        }
        self.selected = Some(index);
        Ok(())
    }

    /// Chooses the first and last financial year to pull.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] outside the choose step, [`ImportError::Period`] when `first`
    /// is after `last`. The choice is unchanged on error.
    pub fn set_years(
        &mut self,
        first: FinancialYear,
        last: FinancialYear,
    ) -> Result<(), ImportError> {
        if self.step != Step::ChooseCompany {
            return Err(self.wrong_step("choose the years"));
        }
        let last_day = last.end()?;
        Period::new(first.start, last_day)?;
        self.first_fy = first;
        self.last_fy = last;
        Ok(())
    }

    // ---- pull ----

    /// Starts the pull and moves to the pull step.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] outside the choose step, [`ImportError::NoCompany`] without a
    /// chosen company, [`ImportError::Period`] for an invalid period.
    pub fn start_pull(&mut self) -> Result<(), ImportError> {
        if self.step != Step::ChooseCompany {
            return Err(self.wrong_step("start the pull"));
        }
        let company = self.selected_company()?.to_owned();
        let period = self.period()?;
        self.cancel_token = CancelToken::new();
        self.pull = PullState::Running(PullProgress::starting());
        self.step = Step::Pull;
        self.requests.push(ImportRequest::Pull {
            source: self.source.clone(),
            company,
            period,
            cancel: self.cancel_token.clone(),
        });
        Ok(())
    }

    /// Records progress from the host.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no pull is running.
    pub fn pull_progress(&mut self, progress: PullProgress) -> Result<(), ImportError> {
        match self.pull {
            PullState::Running(_) => self.pull = PullState::Running(progress),
            PullState::Cancelling(_) => self.pull = PullState::Cancelling(progress),
            PullState::Idle
            | PullState::Done(_)
            | PullState::Cancelled { .. }
            | PullState::Failed(_) => return Err(self.wrong_step("record pull progress")),
        }
        Ok(())
    }

    /// The pull finished. When the user had already asked to cancel, the result is discarded:
    /// nothing is imported and the state is [`PullState::Cancelled`].
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no pull is running.
    pub fn pull_finished(&mut self, summary: PullSummary) -> Result<(), ImportError> {
        match self.pull {
            PullState::Running(_) => self.pull = PullState::Done(summary),
            PullState::Cancelling(_) => {
                self.pull = PullState::Cancelled {
                    batches_kept: summary.batches,
                };
            }
            PullState::Idle
            | PullState::Done(_)
            | PullState::Cancelled { .. }
            | PullState::Failed(_) => return Err(self.wrong_step("finish the pull")),
        }
        Ok(())
    }

    /// The pull stopped because it was cancelled.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no pull is running.
    pub fn pull_cancelled(&mut self, batches_kept: u32) -> Result<(), ImportError> {
        if !matches!(self.pull, PullState::Running(_) | PullState::Cancelling(_)) {
            return Err(self.wrong_step("record the cancel"));
        }
        self.pull = PullState::Cancelled { batches_kept };
        Ok(())
    }

    /// The pull failed.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no pull is running.
    pub fn pull_failed(&mut self, reason: String) -> Result<(), ImportError> {
        if !matches!(self.pull, PullState::Running(_) | PullState::Cancelling(_)) {
            return Err(self.wrong_step("record the failure"));
        }
        self.pull = PullState::Failed(reason);
        Ok(())
    }

    /// Asks the running pull to stop. The pull goes on until the host reports it stopped.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no pull is running.
    pub fn cancel(&mut self) -> Result<(), ImportError> {
        match self.pull {
            PullState::Running(progress) => {
                self.cancel_token.cancel();
                self.pull = PullState::Cancelling(progress);
                Ok(())
            }
            PullState::Idle
            | PullState::Cancelling(_)
            | PullState::Done(_)
            | PullState::Cancelled { .. }
            | PullState::Failed(_) => Err(self.wrong_step("cancel")),
        }
    }

    // ---- map ----

    /// Moves from a finished pull to the map step and asks the host to stage and map.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] unless the pull is done.
    pub fn continue_to_map(&mut self) -> Result<(), ImportError> {
        if self.step != Step::Pull || !matches!(self.pull, PullState::Done(_)) {
            return Err(self.wrong_step("continue to mapping"));
        }
        let company = self.selected_company()?.to_owned();
        self.step = Step::Map;
        self.map = MapState::Working;
        self.requests.push(ImportRequest::Map { company });
        Ok(())
    }

    /// The host's mapping.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no map request is out.
    pub fn map_ready(&mut self, report: MapReport) -> Result<(), ImportError> {
        if self.step != Step::Map || self.map != MapState::Working {
            return Err(self.wrong_step("accept the mapping"));
        }
        self.map = MapState::Ready(report);
        Ok(())
    }

    /// Mapping failed.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no map request is out.
    pub fn map_failed(&mut self, reason: String) -> Result<(), ImportError> {
        if self.step != Step::Map || self.map != MapState::Working {
            return Err(self.wrong_step("record the mapping failure"));
        }
        self.map = MapState::Failed(reason);
        Ok(())
    }

    /// Accepts the mapping and asks the host to write it to the books.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] unless the mapping is ready, [`ImportError::UnmappedItems`]
    /// while any item has no mapping.
    pub fn confirm_map(&mut self) -> Result<(), ImportError> {
        let MapState::Ready(report) = &self.map else {
            return Err(self.wrong_step("confirm the mapping"));
        };
        let unmapped = report.unmapped_count();
        if unmapped > 0 {
            return Err(ImportError::UnmappedItems { count: unmapped });
        }
        let company = self.selected_company()?.to_owned();
        self.step = Step::Reconcile;
        self.recon = ReconState::Committing;
        self.requests.push(ImportRequest::Commit { company });
        Ok(())
    }

    // ---- reconcile ----

    /// The data is in the books; asks the host to compare reports.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no commit is out.
    pub fn commit_done(&mut self) -> Result<(), ImportError> {
        if self.step != Step::Reconcile || self.recon != ReconState::Committing {
            return Err(self.wrong_step("record the commit"));
        }
        self.recon = ReconState::Reconciling;
        self.push_reconcile()
    }

    /// The write failed. Nothing was written (rule D3); the user can go back to the mapping.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no commit is out.
    pub fn commit_failed(&mut self, reason: String) -> Result<(), ImportError> {
        if self.step != Step::Reconcile || self.recon != ReconState::Committing {
            return Err(self.wrong_step("record the commit failure"));
        }
        self.recon = ReconState::CommitFailed(reason);
        Ok(())
    }

    /// The host's reconciliation report.
    ///
    /// The report must cover the period the wizard asked for. A report for any other period is
    /// refused and the comparison stays out, so a late reply for another year cannot show as a
    /// full match.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no comparison is out, and
    /// [`ImportError::WrongPeriod`] when the report covers another period.
    pub fn recon_ready(&mut self, report: ReconReport) -> Result<(), ImportError> {
        if self.step != Step::Reconcile || self.recon != ReconState::Reconciling {
            return Err(self.wrong_step("accept the reconciliation"));
        }
        let asked = self.period()?;
        if report.period != asked {
            return Err(ImportError::WrongPeriod {
                asked,
                got: report.period,
            });
        }
        self.recon = ReconState::Ready(report);
        Ok(())
    }

    /// The comparison failed. The data is in the books.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when no comparison is out.
    pub fn recon_failed(&mut self, reason: String) -> Result<(), ImportError> {
        if self.step != Step::Reconcile || self.recon != ReconState::Reconciling {
            return Err(self.wrong_step("record the reconciliation failure"));
        }
        self.recon = ReconState::Failed(reason);
        Ok(())
    }

    /// Runs the comparison again after a failure.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] unless the last comparison failed.
    pub fn retry_reconcile(&mut self) -> Result<(), ImportError> {
        if self.step != Step::Reconcile || !matches!(self.recon, ReconState::Failed(_)) {
            return Err(self.wrong_step("run the comparison again"));
        }
        self.recon = ReconState::Reconciling;
        self.push_reconcile()
    }

    fn push_reconcile(&mut self) -> Result<(), ImportError> {
        let company = self.selected_company()?.to_owned();
        let period = self.period()?;
        self.requests.push(ImportRequest::Reconcile {
            company,
            period,
            cancel: self.cancel_token.clone(),
        });
        Ok(())
    }

    // ---- going back ----

    /// Goes back one step where that loses nothing: from choose to detect, from a stopped or
    /// finished pull to choose, from the mapping to the pull, and from a failed commit to the
    /// mapping. After a successful commit there is no way back.
    ///
    /// # Errors
    /// [`ImportError::WrongStep`] when going back is not possible from here.
    pub fn back(&mut self) -> Result<(), ImportError> {
        match self.step {
            Step::Detect => Err(self.wrong_step("go back")),
            Step::ChooseCompany => {
                self.step = Step::Detect;
                self.detect = DetectState::NotChecked;
                Ok(())
            }
            Step::Pull => match self.pull {
                PullState::Done(_) | PullState::Cancelled { .. } | PullState::Failed(_) => {
                    self.pull = PullState::Idle;
                    self.step = Step::ChooseCompany;
                    Ok(())
                }
                PullState::Idle | PullState::Running(_) | PullState::Cancelling(_) => {
                    Err(self.wrong_step("go back"))
                }
            },
            Step::Map => match self.map {
                MapState::Ready(_) | MapState::Failed(_) => {
                    self.map = MapState::NotStarted;
                    self.step = Step::Pull;
                    Ok(())
                }
                MapState::NotStarted | MapState::Working => Err(self.wrong_step("go back")),
            },
            Step::Reconcile => match self.recon {
                ReconState::CommitFailed(_) => {
                    self.recon = ReconState::NotStarted;
                    self.step = Step::Map;
                    Ok(())
                }
                ReconState::NotStarted
                | ReconState::Committing
                | ReconState::Reconciling
                | ReconState::Failed(_)
                | ReconState::Ready(_) => Err(self.wrong_step("go back")),
            },
        }
    }
}
